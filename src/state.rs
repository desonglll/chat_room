//! Shared room state backed by SQLite with in-memory broadcast channels.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use tokio::sync::{broadcast, RwLock};
use uuid::Uuid;

use crate::admin_metrics::RuntimeMetrics;
use crate::ai::AiAssistant;
use crate::attachment_content::ContentHashLocks;
use crate::attachment_storage::{self, AttachmentStore};
use crate::attachments::upload_hashes::UploadHashTracker;
use crate::cache::SessionCache;
use crate::config::AppConfig;
use crate::models::{ChatMessage, Room, RoomMember, User};
use crate::social::rate_limits::SocialRateLimits;
use crate::storage;

const SELECT_ROOMS: &str = "SELECT id, name, password_hash, \
     password_hash <> '' AS has_password, creator_user_id, join_policy, \
     avatar_emoji, description, \
     CAST(NULL AS TEXT) AS membership_status, CAST(NULL AS TEXT) AS membership_role, \
     CAST(0 AS BIGINT) AS unread_count, created_at FROM rooms WHERE deleted_at IS NULL";

macro_rules! with_pool {
    ($state:expr, |$pool:ident| $body:block) => {
        match $state.database_pool() {
            $crate::storage::DatabasePool::Sqlite($pool) => $body,
            $crate::storage::DatabasePool::Postgres($pool) => $body,
        }
    };
}
pub(crate) use with_pool;

#[derive(Clone)]
pub(crate) enum RoomEvent {
    Message(Box<ChatMessage>),
    Disconnect { reason: String },
    DisconnectUser { user_id: Uuid, reason: String },
}

struct RoomChannel {
    tx: broadcast::Sender<RoomEvent>,
}

struct ConnectedMember {
    member: RoomMember,
    connections: usize,
}

impl RoomChannel {
    fn new() -> Self {
        let (tx, _) = broadcast::channel(256);
        Self { tx }
    }
}

/// Application state. SQLite is durable storage; the room map is a read cache.
pub struct AppState {
    pub(crate) pool: storage::DatabasePool,
    rooms: RwLock<HashMap<Uuid, Room>>,
    channels: RwLock<HashMap<Uuid, RoomChannel>>,
    members: RwLock<HashMap<Uuid, HashMap<Uuid, ConnectedMember>>>,
    pub(crate) max_upload_bytes: usize,
    pub(crate) attachment_store: AttachmentStore,
    pub(crate) content_hash_locks: ContentHashLocks,
    pub(crate) upload_hashes: UploadHashTracker,
    pub(crate) runtime_metrics: RuntimeMetrics,
    /// Per-(room, from, target) cooldown timestamps for rate-limited, ephemeral
    /// actions (poke, AI suggestions) that don't need database persistence.
    action_cooldowns: RwLock<HashMap<(Uuid, Uuid, Uuid), Instant>>,
    pub(crate) social_rate_limits: SocialRateLimits,
    pub(crate) ai_assistant: Option<AiAssistant>,
    pub(crate) config: AppConfig,
    pub(crate) session_cache: Option<SessionCache>,
}

impl AppState {
    /// Open a database file, creating it and applying migrations automatically.
    pub async fn open(database_path: &Path) -> Result<Self> {
        Self::open_with_config(database_path, &AppConfig::default()).await
    }

    /// Open a database with validated runtime settings.
    pub async fn open_with_config(database_path: &Path, config: &AppConfig) -> Result<Self> {
        let attachment_store = open_attachment_store(config).await?;
        let pool = storage::open_database(database_path, &attachment_store).await?;
        Self::from_pool(
            storage::DatabasePool::Sqlite(pool),
            config.max_upload_bytes()?,
            attachment_store,
            ai_assistant_for(config),
            config.clone(),
        )
        .await
    }

    pub async fn open_postgres(url: &str, config: &AppConfig) -> Result<Self> {
        let attachment_store = open_attachment_store(config).await?;
        let pool = storage::open_postgres_database(url, config.database.max_connections).await?;
        Self::from_pool(
            pool,
            config.max_upload_bytes()?,
            attachment_store,
            ai_assistant_for(config),
            config.clone(),
        )
        .await
    }

    /// Open the configured database or the default chat_rooms.db file.
    pub async fn load(storage_path: Option<String>) -> Result<Self> {
        let path = storage_path.as_deref().unwrap_or("chat_rooms.db");
        Self::open(Path::new(path)).await
    }

    /// Create an isolated in-memory database for tests.
    pub async fn new() -> Result<Self> {
        Self::new_with_config(&AppConfig::default()).await
    }

    /// Create an isolated in-memory database with explicit runtime settings.
    pub async fn new_with_config(config: &AppConfig) -> Result<Self> {
        let attachment_store = AttachmentStore::open(
            attachment_storage::test_directory(),
            gc_age(config),
            &config.attachments.oss,
        )
        .await?;
        let pool = storage::open_memory_database(&attachment_store).await?;
        Self::from_pool(
            storage::DatabasePool::Sqlite(pool),
            config.max_upload_bytes()?,
            attachment_store,
            ai_assistant_for(config),
            config.clone(),
        )
        .await
    }

    async fn from_pool(
        pool: storage::DatabasePool,
        max_upload_bytes: usize,
        attachment_store: AttachmentStore,
        ai_assistant: Option<AiAssistant>,
        config: AppConfig,
    ) -> Result<Self> {
        let loaded: Vec<Room> = match &pool {
            storage::DatabasePool::Sqlite(database) => {
                sqlx::query_as(SELECT_ROOMS).fetch_all(database).await
            }
            storage::DatabasePool::Postgres(database) => {
                sqlx::query_as(SELECT_ROOMS).fetch_all(database).await
            }
        }
        .context("load rooms from database")?;
        let session_cache = if config.redis.enabled {
            match SessionCache::connect(&config.redis).await {
                Ok(cache) => {
                    tracing::info!("Redis session cache enabled");
                    Some(cache)
                }
                Err(error) => {
                    tracing::warn!("Redis unavailable; using database sessions: {error:#}");
                    None
                }
            }
        } else {
            None
        };

        let mut rooms = HashMap::with_capacity(loaded.len());
        let mut channels = HashMap::with_capacity(loaded.len());
        for room in loaded {
            channels.insert(room.id, RoomChannel::new());
            rooms.insert(room.id, room);
        }

        let state = Self {
            pool,
            rooms: RwLock::new(rooms),
            channels: RwLock::new(channels),
            members: RwLock::new(HashMap::new()),
            max_upload_bytes,
            attachment_store,
            content_hash_locks: ContentHashLocks::default(),
            upload_hashes: UploadHashTracker::default(),
            runtime_metrics: RuntimeMetrics::default(),
            action_cooldowns: RwLock::new(HashMap::new()),
            social_rate_limits: SocialRateLimits::default(),
            ai_assistant,
            config,
            session_cache,
        };
        state.backfill_attachment_content_hashes().await?;
        Ok(state)
    }

    /// Returns true (and starts a new cooldown window) if enough time has passed
    /// since the last time this exact (room, from, target) action fired.
    pub(crate) async fn check_action_cooldown(
        &self,
        room_id: Uuid,
        from: Uuid,
        target: Uuid,
        window: Duration,
    ) -> bool {
        let key = (room_id, from, target);
        let now = Instant::now();
        let mut cooldowns = self.action_cooldowns.write().await;
        if let Some(last) = cooldowns.get(&key) {
            if now.duration_since(*last) < window {
                return false;
            }
        }
        cooldowns.insert(key, now);
        true
    }

    pub(crate) async fn cache_inserted_room(&self, room: Room) {
        let id = room.id;
        self.rooms.write().await.insert(id, room);
        self.channels.write().await.insert(id, RoomChannel::new());
    }

    pub(crate) async fn cache_updated_room(&self, room: Room) {
        self.rooms.write().await.insert(room.id, room);
    }

    pub(crate) async fn remove_cached_room(&self, id: Uuid, reason: &str) {
        self.rooms.write().await.remove(&id);
        self.disconnect_room(id, reason).await;
    }

    /// Return rooms in stable creation order, optionally filtered by exact name.
    pub async fn list_rooms(&self, name: Option<&str>) -> Vec<Room> {
        let rooms = self.rooms.read().await;
        let mut list: Vec<Room> = rooms
            .values()
            .filter(|room| name.is_none_or(|wanted| room.name == wanted))
            .cloned()
            .collect();
        list.sort_by(|left, right| {
            left.created_at
                .cmp(&right.created_at)
                .then_with(|| left.id.cmp(&right.id))
        });
        list
    }

    pub async fn room(&self, id: Uuid) -> Option<Room> {
        self.rooms.read().await.get(&id).cloned()
    }

    pub(crate) async fn subscribe(&self, id: Uuid) -> Option<broadcast::Receiver<RoomEvent>> {
        self.channels
            .read()
            .await
            .get(&id)
            .map(|room| room.tx.subscribe())
    }

    pub async fn broadcast(&self, id: Uuid, message: ChatMessage) {
        if let Some(room) = self.channels.read().await.get(&id) {
            let _ = room.tx.send(RoomEvent::Message(Box::new(message)));
        }
    }

    /// Track unique accounts while allowing the same account to use multiple tabs.
    pub async fn member_connected(&self, room_id: Uuid, user: &User) -> (Vec<RoomMember>, bool) {
        let mut rooms = self.members.write().await;
        let room = rooms.entry(room_id).or_default();
        let first_connection = !room.contains_key(&user.id);
        let connected = room.entry(user.id).or_insert_with(|| ConnectedMember {
            member: RoomMember {
                user_id: user.id,
                username: user.username.clone(),
                avatar_emoji: user.avatar_emoji.clone(),
            },
            connections: 0,
        });
        connected.connections += 1;
        (sorted_members(room), first_connection)
    }

    /// Remove one socket and report whether the account fully left the room.
    pub async fn member_disconnected(
        &self,
        room_id: Uuid,
        user_id: Uuid,
    ) -> (Vec<RoomMember>, bool) {
        let mut rooms = self.members.write().await;
        let Some(room) = rooms.get_mut(&room_id) else {
            return (Vec::new(), false);
        };
        let fully_disconnected = match room.get_mut(&user_id) {
            Some(connected) if connected.connections > 1 => {
                connected.connections -= 1;
                false
            }
            Some(_) => {
                room.remove(&user_id);
                true
            }
            None => false,
        };
        let members = sorted_members(room);
        if room.is_empty() {
            rooms.remove(&room_id);
        }
        (members, fully_disconnected)
    }

    pub async fn remove_connected_member(&self, room_id: Uuid, user_id: Uuid) -> Vec<RoomMember> {
        let mut rooms = self.members.write().await;
        let Some(room) = rooms.get_mut(&room_id) else {
            return Vec::new();
        };
        room.remove(&user_id);
        let members = sorted_members(room);
        if room.is_empty() {
            rooms.remove(&room_id);
        }
        members
    }

    pub async fn connected_members(&self, room_id: Uuid) -> Vec<RoomMember> {
        self.members
            .read()
            .await
            .get(&room_id)
            .map(sorted_members)
            .unwrap_or_default()
    }

    pub async fn disconnect_room_member(&self, id: Uuid, user_id: Uuid, reason: &str) {
        if let Some(room) = self.channels.read().await.get(&id) {
            let _ = room.tx.send(RoomEvent::DisconnectUser {
                user_id,
                reason: reason.to_string(),
            });
        }
    }

    pub async fn disconnect_all_chat_rooms(&self, reason: &str) {
        for room in self.channels.read().await.values() {
            let _ = room.tx.send(RoomEvent::Disconnect {
                reason: reason.to_string(),
            });
        }
    }

    /// Refresh a connected account in every room and publish the new member snapshots.
    pub async fn publish_member_profile(&self, user: &User) {
        let snapshots = {
            let mut rooms = self.members.write().await;
            let mut snapshots = Vec::new();
            for (room_id, members) in rooms.iter_mut() {
                let Some(connected) = members.get_mut(&user.id) else {
                    continue;
                };
                connected.member.username = user.username.clone();
                connected.member.avatar_emoji = user.avatar_emoji.clone();
                snapshots.push((*room_id, sorted_members(members)));
            }
            snapshots
        };

        for (room_id, members) in snapshots {
            let participants = match self.room_participants(room_id).await {
                Ok(participants) => participants,
                Err(error) => {
                    tracing::warn!(
                        "load room participants for profile update failed: {}",
                        error
                    );
                    Vec::new()
                }
            };
            self.broadcast(
                room_id,
                ChatMessage::Presence {
                    members,
                    participants,
                },
            )
            .await;
        }
    }

    /// Close current room sessions and install a fresh channel for future joins.
    pub async fn restart_room_connections(&self, id: Uuid, reason: &str) {
        let previous = self.channels.write().await.insert(id, RoomChannel::new());
        if let Some(previous) = previous {
            let _ = previous.tx.send(RoomEvent::Disconnect {
                reason: reason.to_string(),
            });
        }
    }

    async fn disconnect_room(&self, id: Uuid, reason: &str) {
        if let Some(room) = self.channels.write().await.remove(&id) {
            let _ = room.tx.send(RoomEvent::Disconnect {
                reason: reason.to_string(),
            });
        }
    }

    pub(crate) async fn online_counts(&self) -> (u64, u64) {
        let rooms = self.members.read().await;
        let mut users = HashSet::new();
        let mut connections = 0u64;
        for members in rooms.values() {
            for (user_id, connected) in members {
                users.insert(*user_id);
                connections += connected.connections as u64;
            }
        }
        (users.len() as u64, connections)
    }
}

/// Convenience alias used by axum handlers.
pub type SharedState = Arc<AppState>;

fn ai_assistant_for(config: &AppConfig) -> Option<AiAssistant> {
    config.ai.enabled.then(|| AiAssistant::new(&config.ai))
}

fn gc_age(config: &AppConfig) -> Duration {
    Duration::from_secs(
        config
            .uploads
            .abandoned_upload_gc_hours
            .saturating_mul(3600),
    )
}

async fn open_attachment_store(config: &AppConfig) -> Result<AttachmentStore> {
    AttachmentStore::open(
        &config.attachments.directory,
        gc_age(config),
        &config.attachments.oss,
    )
    .await
}

fn sorted_members(members: &HashMap<Uuid, ConnectedMember>) -> Vec<RoomMember> {
    let mut result: Vec<_> = members
        .values()
        .map(|connected| connected.member.clone())
        .collect();
    result.sort_by(|left, right| {
        left.username
            .to_lowercase()
            .cmp(&right.username.to_lowercase())
            .then_with(|| left.user_id.cmp(&right.user_id))
    });
    result
}
