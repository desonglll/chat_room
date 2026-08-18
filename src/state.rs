//! Shared room state backed by SQLite with in-memory broadcast channels.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, Result};
use sqlx::SqlitePool;
use tokio::sync::{broadcast, RwLock};
use uuid::Uuid;

use crate::config::AppConfig;
use crate::models::{ChatMessage, Room, RoomMember, User};
use crate::storage;

const SELECT_ROOMS: &str = "SELECT id, name, password_hash, \
     password_hash <> '' AS has_password, creator_user_id, join_policy, \
     NULL AS membership_status, NULL AS membership_role, 0 AS unread_count, created_at FROM rooms";

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
    pool: SqlitePool,
    rooms: RwLock<HashMap<Uuid, Room>>,
    channels: RwLock<HashMap<Uuid, RoomChannel>>,
    members: RwLock<HashMap<Uuid, HashMap<Uuid, ConnectedMember>>>,
    max_upload_bytes: usize,
}

impl AppState {
    /// Open a database file, creating it and applying migrations automatically.
    pub async fn open(database_path: &Path) -> Result<Self> {
        Self::open_with_config(database_path, &AppConfig::default()).await
    }

    /// Open a database with validated runtime settings.
    pub async fn open_with_config(database_path: &Path, config: &AppConfig) -> Result<Self> {
        let pool = storage::open_database(database_path).await?;
        Self::from_pool(pool, config.max_upload_bytes()?).await
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
        let pool = storage::open_memory_database().await?;
        Self::from_pool(pool, config.max_upload_bytes()?).await
    }

    async fn from_pool(pool: SqlitePool, max_upload_bytes: usize) -> Result<Self> {
        let loaded: Vec<Room> = sqlx::query_as(SELECT_ROOMS)
            .fetch_all(&pool)
            .await
            .context("load rooms from SQLite")?;

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
        })
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
        let result = sqlx::query(
            "UPDATE rooms SET name = ?, password_hash = ?, join_policy = ? \
             WHERE id = ? AND name = ? AND password_hash = ? AND join_policy = ?",
        )
        .bind(&updated.name)
        .bind(&updated.password_hash)
        .bind(&updated.join_policy)
        .bind(previous.id)
        .bind(&previous.name)
        .bind(&previous.password_hash)
        .bind(&previous.join_policy)
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Ok(false);
        }

        self.rooms.write().await.insert(updated.id, updated);
        Ok(true)
    }

    /// Delete a room if its password has not changed since authorization.
    pub async fn delete_room(
        &self,
        id: Uuid,
        expected_password_hash: &str,
    ) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("DELETE FROM rooms WHERE id = ? AND password_hash = ?")
            .bind(id)
            .bind(expected_password_hash)
            .execute(&self.pool)
            .await?;

        if result.rows_affected() == 0 {
            return Ok(false);
        }

        self.rooms.write().await.remove(&id);
        self.disconnect_room(id, "room deleted").await;
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
        &self.pool
    }

    pub fn max_upload_bytes(&self) -> usize {
        self.max_upload_bytes
    }
}

/// Convenience alias used by axum handlers.
pub type SharedState = Arc<AppState>;

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
