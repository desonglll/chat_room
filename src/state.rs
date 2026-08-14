//! Shared room state backed by SQLite with in-memory broadcast channels.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use sqlx::SqlitePool;
use tokio::sync::{broadcast, RwLock};
use uuid::Uuid;

use crate::models::{ChatMessage, Room, StoredMessage};
use crate::storage;

const SELECT_ROOMS: &str = "SELECT id, name, password_hash, \
     password_hash <> '' AS has_password, created_at FROM rooms";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MessageCursor {
    pub created_at: DateTime<Utc>,
    pub id: Uuid,
}

#[derive(Clone)]
pub(crate) enum RoomEvent {
    Message(ChatMessage),
    Disconnect { reason: String },
}

struct RoomChannel {
    tx: broadcast::Sender<RoomEvent>,
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
}

impl AppState {
    /// Open a database file, creating it and applying migrations automatically.
    pub async fn open(database_path: &Path) -> Result<Self> {
        let pool = storage::open_database(database_path).await?;
        Self::from_pool(pool).await
    }

    /// Open the configured database or the default chat_rooms.db file.
    pub async fn load(storage_path: Option<String>) -> Result<Self> {
        let path = storage_path.as_deref().unwrap_or("chat_rooms.db");
        Self::open(Path::new(path)).await
    }

    /// Create an isolated in-memory database for tests.
    pub async fn new() -> Result<Self> {
        let pool = storage::open_memory_database().await?;
        Self::from_pool(pool).await
    }

    async fn from_pool(pool: SqlitePool) -> Result<Self> {
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
        })
    }

    /// Insert a room transactionally, then update the read cache and channel map.
    pub async fn insert_room(&self, room: Room) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO rooms (id, name, password_hash, created_at) \
             VALUES (?, ?, ?, ?)",
        )
        .bind(room.id)
        .bind(&room.name)
        .bind(&room.password_hash)
        .bind(room.created_at)
        .execute(&self.pool)
        .await?;

        let id = room.id;
        self.rooms.write().await.insert(id, room);
        self.channels.write().await.insert(id, RoomChannel::new());
        Ok(())
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
            "UPDATE rooms SET name = ?, password_hash = ? \
             WHERE id = ? AND name = ? AND password_hash = ?",
        )
        .bind(&updated.name)
        .bind(&updated.password_hash)
        .bind(previous.id)
        .bind(&previous.name)
        .bind(&previous.password_hash)
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

    /// Store a user message before it is broadcast to room participants.
    pub async fn store_message(
        &self,
        room_id: Uuid,
        sender_id: Uuid,
        sender: &str,
        content: &str,
    ) -> Result<StoredMessage, sqlx::Error> {
        let id = Uuid::new_v4();
        let created_at = Utc::now();
        sqlx::query(
            "INSERT INTO messages (id, room_id, sender_id, sender, content, created_at) \
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(id)
        .bind(room_id)
        .bind(sender_id)
        .bind(sender)
        .bind(content)
        .bind(created_at)
        .execute(&self.pool)
        .await?;

        Ok(StoredMessage {
            id,
            room_id,
            sender_id: Some(sender_id),
            sender: sender.to_string(),
            content: content.to_string(),
            created_at,
        })
    }

    pub(crate) async fn latest_message_cursor(
        &self,
        room_id: Uuid,
    ) -> Result<Option<MessageCursor>, sqlx::Error> {
        let row: Option<(DateTime<Utc>, Uuid)> = sqlx::query_as(
            "SELECT created_at, id FROM messages WHERE room_id = ? \
             ORDER BY created_at DESC, id DESC LIMIT 1",
        )
        .bind(room_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|(created_at, id)| MessageCursor { created_at, id }))
    }

    /// Return messages created after a connection's database cursor.
    pub(crate) async fn messages_after(
        &self,
        room_id: Uuid,
        cursor: Option<&MessageCursor>,
        limit: i64,
    ) -> Result<Vec<StoredMessage>, sqlx::Error> {
        let limit = limit.clamp(1, 500);
        match cursor {
            Some(cursor) => {
                sqlx::query_as(
                    "SELECT id, room_id, sender_id, sender, content, created_at \
                     FROM messages WHERE room_id = ? AND \
                     (created_at > ? OR (created_at = ? AND id > ?)) \
                     ORDER BY created_at ASC, id ASC LIMIT ?",
                )
                .bind(room_id)
                .bind(cursor.created_at)
                .bind(cursor.created_at)
                .bind(cursor.id)
                .bind(limit)
                .fetch_all(&self.pool)
                .await
            }
            None => {
                sqlx::query_as(
                    "SELECT id, room_id, sender_id, sender, content, created_at \
                     FROM messages WHERE room_id = ? \
                     ORDER BY created_at ASC, id ASC LIMIT ?",
                )
                .bind(room_id)
                .bind(limit)
                .fetch_all(&self.pool)
                .await
            }
        }
    }

    /// Return persisted messages oldest-first, limited to the newest entries.
    pub(crate) async fn message_history(
        &self,
        room_id: Uuid,
        limit: i64,
        through: Option<&MessageCursor>,
    ) -> Result<Vec<StoredMessage>, sqlx::Error> {
        let limit = limit.clamp(1, 500);
        let mut messages: Vec<StoredMessage> = match through {
            Some(cursor) => {
                sqlx::query_as(
                    "SELECT id, room_id, sender_id, sender, content, created_at \
                     FROM messages WHERE room_id = ? AND \
                     (created_at < ? OR (created_at = ? AND id <= ?)) \
                     ORDER BY created_at DESC, id DESC LIMIT ?",
                )
                .bind(room_id)
                .bind(cursor.created_at)
                .bind(cursor.created_at)
                .bind(cursor.id)
                .bind(limit)
                .fetch_all(&self.pool)
                .await?
            }
            None => {
                sqlx::query_as(
                    "SELECT id, room_id, sender_id, sender, content, created_at \
                     FROM messages WHERE room_id = ? \
                     ORDER BY created_at DESC, id DESC LIMIT ?",
                )
                .bind(room_id)
                .bind(limit)
                .fetch_all(&self.pool)
                .await?
            }
        };
        messages.reverse();
        Ok(messages)
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
            let _ = room.tx.send(RoomEvent::Message(message));
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
}

/// Convenience alias used by axum handlers.
pub type SharedState = Arc<AppState>;
