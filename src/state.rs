//! Shared room state backed by SQLite with in-memory broadcast channels.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use chrono::Utc;
use sqlx::{PgPool, SqlitePool};
use tokio::sync::{broadcast, RwLock};
use uuid::Uuid;

use crate::ai::AiAssistant;
use crate::attachment_storage::{self, AttachmentStore};
use crate::config::AppConfig;
use crate::models::{ChatMessage, Room, RoomMember, User};
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
    pool: storage::DatabasePool,
    rooms: RwLock<HashMap<Uuid, Room>>,
    channels: RwLock<HashMap<Uuid, RoomChannel>>,
    members: RwLock<HashMap<Uuid, HashMap<Uuid, ConnectedMember>>>,
    max_upload_bytes: usize,
    attachment_store: AttachmentStore,
    /// Per-(room, from, target) cooldown timestamps for rate-limited, ephemeral
    /// actions (poke, AI suggestions) that don't need database persistence.
    action_cooldowns: RwLock<HashMap<(Uuid, Uuid, Uuid), Instant>>,
    ai_assistant: Option<AiAssistant>,
    config: AppConfig,
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

        let mut rooms = HashMap::with_capacity(loaded.len());
        let mut channels = HashMap::with_capacity(loaded.len());
        for room in loaded {
            channels.insert(room.id, RoomChannel::new());
            rooms.insert(room.id, room);
        }

        Ok(Self {
            pool,
            rooms: RwLock::new(rooms),
            channels: RwLock::new(channels),
            members: RwLock::new(HashMap::new()),
            max_upload_bytes,
            attachment_store,
            action_cooldowns: RwLock::new(HashMap::new()),
            ai_assistant,
            config,
        })
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

    /// Persist a room edit only if the caller's view is still current.
    pub async fn update_room(&self, previous: &Room, updated: Room) -> Result<bool, sqlx::Error> {
        let changed = with_pool!(self, |pool| {
            sqlx::query(
                "UPDATE rooms SET name = $1, password_hash = $2, join_policy = $3, \
                 avatar_emoji = $4, description = $5 \
                 WHERE id = $6 AND name = $7 AND password_hash = $8 AND join_policy = $9",
            )
            .bind(&updated.name)
            .bind(&updated.password_hash)
            .bind(&updated.join_policy)
            .bind(&updated.avatar_emoji)
            .bind(&updated.description)
            .bind(previous.id)
            .bind(&previous.name)
            .bind(&previous.password_hash)
            .bind(&previous.join_policy)
            .execute(pool)
            .await
            .map(|result| result.rows_affected())
        })?;

        if changed == 0 {
            return Ok(false);
        }

        self.rooms.write().await.insert(updated.id, updated);
        Ok(true)
    }

    /// Soft-delete a room if its password has not changed since authorization: the
    /// room disappears from listings, but messages/attachments stay intact so the
    /// deletion is reviewable/recoverable later rather than destructive.
    pub async fn delete_room(
        &self,
        id: Uuid,
        expected_password_hash: &str,
    ) -> Result<bool, sqlx::Error> {
        let changed = with_pool!(self, |pool| {
            sqlx::query(
                "UPDATE rooms SET deleted_at = $1 \
                 WHERE id = $2 AND password_hash = $3 AND deleted_at IS NULL",
            )
            .bind(Utc::now())
            .bind(id)
            .bind(expected_password_hash)
            .execute(pool)
            .await
            .map(|result| result.rows_affected())
        })?;

        if changed == 0 {
            return Ok(false);
        }

        self.rooms.write().await.remove(&id);
        self.disconnect_room(id, "room deleted").await;
        Ok(true)
    }

    /// Permanently remove a room and its attachments. Not currently exposed by any
    /// handler — soft-delete (`delete_room`) is the only user-facing deletion path —
    /// but kept available for a future admin purge/retention job.
    #[allow(dead_code)]
    async fn hard_delete_room(
        &self,
        id: Uuid,
        expected_password_hash: &str,
    ) -> Result<bool, sqlx::Error> {
        let (attachment_ids, changed) = with_pool!(self, |pool| {
            let attachment_ids: Vec<Uuid> =
            sqlx::query_scalar("SELECT id FROM attachments WHERE room_id = $1")
                .bind(id)
                .fetch_all(pool)
                .await?;
            let result = sqlx::query("DELETE FROM rooms WHERE id = $1 AND password_hash = $2")
                .bind(id)
                .bind(expected_password_hash)
                .execute(pool)
                .await?;
            Ok::<_, sqlx::Error>((attachment_ids, result.rows_affected()))
        })?;

        if changed == 0 {
            return Ok(false);
        }

        self.rooms.write().await.remove(&id);
        self.disconnect_room(id, "room deleted").await;
        for attachment_id in attachment_ids {
            if let Err(error) = self.attachment_store.remove(attachment_id).await {
                tracing::warn!("remove deleted room attachment failed: {error:#}");
            }
        }
        Ok(true)
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

    pub fn pool(&self) -> &SqlitePool {
        match &self.pool {
            storage::DatabasePool::Sqlite(pool) => pool,
            storage::DatabasePool::Postgres(_) => panic!("SQLite pool requested for PostgreSQL state"),
        }
    }

    pub fn postgres_pool(&self) -> Option<&PgPool> {
        match &self.pool {
            storage::DatabasePool::Postgres(pool) => Some(pool),
            storage::DatabasePool::Sqlite(_) => None,
        }
    }

    pub(crate) fn database_pool(&self) -> &storage::DatabasePool {
        &self.pool
    }

    pub fn max_upload_bytes(&self) -> usize {
        self.max_upload_bytes
    }

    pub fn ai_enabled(&self) -> bool {
        self.ai_assistant.is_some()
    }

    pub(crate) fn ai_assistant(&self) -> Option<&AiAssistant> {
        self.ai_assistant.as_ref()
    }

    pub(crate) fn ai_max_context_messages(&self) -> usize {
        self.config.ai.max_context_messages
    }

    pub(crate) fn ai_suggest_cooldown(&self) -> Duration {
        Duration::from_secs(self.config.ai.suggest_cooldown_secs)
    }

    pub(crate) fn realtime_config(&self) -> &crate::config::RealtimeConfig {
        &self.config.realtime
    }

    pub(crate) fn poke_cooldown(&self) -> Duration {
        Duration::from_secs(self.config.realtime.poke_cooldown_secs)
    }

    pub(crate) fn session_lifetime_days(&self) -> i64 {
        self.config.auth.session_lifetime_days
    }

    /// Body-size cap for a single resumable-upload chunk request.
    pub fn chunk_body_limit_bytes(&self) -> usize {
        self.config.chunk_size_bytes().unwrap_or(8 * 1024 * 1024)
    }

    pub fn attachment_store(&self) -> &AttachmentStore {
        &self.attachment_store
    }
}

/// Convenience alias used by axum handlers.
pub type SharedState = Arc<AppState>;

fn ai_assistant_for(config: &AppConfig) -> Option<AiAssistant> {
    config.ai.enabled.then(|| AiAssistant::new(&config.ai))
}

fn gc_age(config: &AppConfig) -> Duration {
    Duration::from_secs(config.uploads.abandoned_upload_gc_hours.saturating_mul(3600))
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
