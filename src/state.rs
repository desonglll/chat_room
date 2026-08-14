//! Shared room state backed by SQLite with in-memory broadcast channels.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result};
use sqlx::SqlitePool;
use tokio::sync::{broadcast, RwLock};
use uuid::Uuid;

use crate::models::{ChatMessage, Room};
use crate::storage;

const SELECT_ROOMS: &str = "SELECT id, name, password_hash, \
     password_hash <> '' AS has_password, created_at FROM rooms";

struct RoomChannel {
    tx: broadcast::Sender<ChatMessage>,
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
    pub async fn open(database_path: &Path, legacy_json_path: Option<&Path>) -> Result<Self> {
        let pool = storage::open_database(database_path, legacy_json_path).await?;
        Self::from_pool(pool).await
    }

    /// Compatibility entry point. None uses chat_rooms.db and imports the old
    /// chat_rooms.json file. A .json path is treated as a legacy source.
    pub async fn load(storage_path: Option<String>) -> Result<Self> {
        match storage_path {
            Some(path)
                if Path::new(&path)
                    .extension()
                    .is_some_and(|ext| ext == "json") =>
            {
                let legacy = PathBuf::from(path);
                let database = legacy.with_extension("db");
                Self::open(&database, Some(&legacy)).await
            }
            Some(path) => Self::open(Path::new(&path), None).await,
            None => {
                Self::open(
                    Path::new("chat_rooms.db"),
                    Some(Path::new("chat_rooms.json")),
                )
                .await
            }
        }
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

    pub async fn subscribe(&self, id: Uuid) -> Option<broadcast::Receiver<ChatMessage>> {
        self.channels
            .read()
            .await
            .get(&id)
            .map(|room| room.tx.subscribe())
    }

    pub async fn broadcast(&self, id: Uuid, message: ChatMessage) {
        if let Some(room) = self.channels.read().await.get(&id) {
            let _ = room.tx.send(message);
        }
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }
}

/// Convenience alias used by axum handlers.
pub type SharedState = Arc<AppState>;
