//! Data models — serialisable structs shared across REST and WebSocket.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

// ── REST models ──────────────────────────────────────────────────────────────

/// Public-facing room descriptor (password hash is never serialised).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
pub struct Room {
    pub id: Uuid,
    pub name: String,
    /// SHA-256 hex digest — empty string means the room is public.
    #[serde(skip_serializing, default)]
    #[schema(value_type = String)]
    pub password_hash: String,
    /// Whether a password is required to join.
    pub has_password: bool,
    pub created_at: DateTime<Utc>,
}

/// Payload for POST /api/rooms.
#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateRoomRequest {
    pub name: String,
    /// Plain-text password — omit or set to "" for a public room.
    #[serde(default)]
    pub password: Option<String>,
}

// ── WebSocket message envelope ───────────────────────────────────────────────

/// Every WebSocket frame carries one JSON-serialised ChatMessage.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ChatMessage {
    /// Client → Server: join a public room (no password needed).
    #[serde(rename = "join")]
    Join { username: String },

    /// Client → Server: authenticate with room password.
    #[serde(rename = "auth")]
    Auth { username: String, password: String },

    /// Server → Client: authentication / join succeeded.
    #[serde(rename = "auth_ok")]
    AuthOk { room_name: String },

    /// Server → Client: authentication / join failed.
    #[serde(rename = "auth_fail")]
    AuthFail { reason: String },

    /// Client → Server: send a chat message.
    #[serde(rename = "message")]
    Message { content: String },

    /// Server → Client: a chat message broadcast from another user.
    #[serde(rename = "broadcast")]
    Broadcast {
        sender: String,
        content: String,
        timestamp: DateTime<Utc>,
    },

    /// Server → Client: system event (join / leave).
    #[serde(rename = "system")]
    System { content: String },
}
