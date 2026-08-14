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

/// A chat message persisted as part of a room session.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
pub struct StoredMessage {
    pub id: Uuid,
    pub room_id: Uuid,
    pub sender_id: Option<Uuid>,
    pub sender: String,
    pub content: String,
    pub created_at: DateTime<Utc>,
}

/// Public account data. Password hashes and session records are never exposed.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
pub struct User {
    pub id: Uuid,
    pub username: String,
    pub created_at: DateTime<Utc>,
}

/// Credentials accepted by registration and login endpoints.
#[derive(Debug, Deserialize, ToSchema)]
pub struct AuthRequest {
    pub username: String,
    pub password: String,
}

/// A login session returned after successful registration or authentication.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AuthSession {
    pub token: Uuid,
    pub user: User,
    pub expires_at: DateTime<Utc>,
}

/// Payload for POST /api/rooms.
#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateRoomRequest {
    pub name: String,
    /// Plain-text password — omit or set to "" for a public room.
    #[serde(default)]
    pub password: Option<String>,
}

/// Payload for PATCH /api/rooms/{id}. Missing fields remain unchanged.
#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateRoomRequest {
    pub name: Option<String>,
    /// Required for every change to a private room.
    #[serde(default)]
    pub current_password: Option<String>,
    /// Set to an empty string to make the room public.
    pub new_password: Option<String>,
}

// ── WebSocket message envelope ───────────────────────────────────────────────

/// Every WebSocket frame carries one JSON-serialised ChatMessage.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ChatMessage {
    /// Client → Server: join a public room (no password needed).
    #[serde(rename = "join")]
    Join { token: Uuid },

    /// Client → Server: authenticate with room password.
    #[serde(rename = "auth")]
    Auth { token: Uuid, password: String },

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
        message_id: Uuid,
        sender_id: Option<Uuid>,
        sender: String,
        content: String,
        timestamp: DateTime<Utc>,
    },

    /// Server → Client: system event (join / leave).
    #[serde(rename = "system")]
    System { content: String },
}
