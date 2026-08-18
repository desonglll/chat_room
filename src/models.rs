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
    pub creator_user_id: Option<Uuid>,
    pub join_policy: String,
    #[serde(skip_deserializing, default, skip_serializing_if = "Option::is_none")]
    #[sqlx(default)]
    pub membership_status: Option<String>,
    #[serde(skip_deserializing, default, skip_serializing_if = "Option::is_none")]
    #[sqlx(default)]
    pub membership_role: Option<String>,
    #[serde(default)]
    #[sqlx(default)]
    pub unread_count: i64,
    pub created_at: DateTime<Utc>,
}

/// A chat message persisted as part of a room session.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct StoredMessage {
    pub id: Uuid,
    pub room_id: Uuid,
    pub sender_id: Option<Uuid>,
    pub sender: String,
    pub sender_avatar: String,
    pub content: String,
    pub attachment: Option<Attachment>,
    pub reply_to: Option<ReplyPreview>,
    pub recalled_at: Option<DateTime<Utc>>,
    pub edited_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

/// Metadata needed to display or download a message attachment.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Attachment {
    pub id: Uuid,
    pub file_name: String,
    pub mime_type: String,
    pub size_bytes: i64,
    pub download_url: String,
}

/// Stable excerpt of the message referenced by a reply.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ReplyPreview {
    pub message_id: Uuid,
    pub sender: String,
    pub content: String,
    pub attachment_file_name: Option<String>,
    pub recalled: bool,
}

/// A unique signed-in account currently connected to a room.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomMember {
    pub user_id: Uuid,
    pub username: String,
    pub avatar_emoji: String,
}

/// The newest message a room participant has viewed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadReceipt {
    pub user_id: Uuid,
    pub username: String,
    pub message_id: Uuid,
}

/// Public account data. Password hashes and session records are never exposed.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
pub struct User {
    pub id: Uuid,
    pub username: String,
    pub avatar_emoji: String,
    pub created_at: DateTime<Utc>,
}

/// Credentials accepted by registration and login endpoints.
#[derive(Debug, Deserialize, ToSchema)]
pub struct AuthRequest {
    pub username: String,
    pub password: String,
}

/// Editable account profile fields.
#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateProfileRequest {
    pub avatar_emoji: String,
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
    #[serde(default)]
    pub join_policy: Option<String>,
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
    pub join_policy: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct JoinRoomRequest {
    #[serde(default)]
    pub password: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct InviteMemberRequest {
    pub username: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateMembershipRequest {
    pub action: String,
    #[serde(default)]
    pub role: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
pub struct RoomMembership {
    pub user_id: Uuid,
    pub username: String,
    pub avatar_emoji: String,
    pub role: String,
    pub status: String,
    pub requested_at: DateTime<Utc>,
    pub joined_at: Option<DateTime<Utc>>,
}

// ── WebSocket message envelope ───────────────────────────────────────────────

/// Every WebSocket frame carries one JSON-serialised ChatMessage.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
// This transport enum intentionally mirrors the JSON protocol without boxed wire fields.
#[allow(clippy::large_enum_variant)]
pub enum ChatMessage {
    /// Client → Server: join a public room (no password needed).
    #[serde(rename = "join")]
    Join { token: Uuid },

    /// Client → Server: authenticate with room password.
    #[serde(rename = "auth")]
    Auth { token: Uuid, password: String },

    /// Server → Client: authentication / join succeeded.
    #[serde(rename = "auth_ok")]
    AuthOk {
        room_name: String,
        members: Vec<RoomMember>,
        participants: Vec<RoomMember>,
        read_receipts: Vec<ReadReceipt>,
    },

    /// Server → Client: authentication / join failed.
    #[serde(rename = "auth_fail")]
    AuthFail { reason: String },

    /// Client → Server: send a chat message.
    #[serde(rename = "message")]
    Message {
        content: String,
        #[serde(default)]
        reply_to: Option<Uuid>,
    },

    /// Client -> Server: replace the content of a message sent by this account.
    #[serde(rename = "edit")]
    Edit { message_id: Uuid, content: String },

    /// Both directions: publish a transient draft to other connected members.
    #[serde(rename = "typing")]
    Typing {
        content: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        user_id: Option<Uuid>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        username: Option<String>,
    },

    /// Client -> Server: advance this account's read position in the room.
    #[serde(rename = "read")]
    Read { message_id: Uuid },

    /// Client -> Server: recall a message sent by this account.
    #[serde(rename = "recall")]
    Recall { message_id: Uuid },

    /// Server → Client: a chat message broadcast from another user.
    #[serde(rename = "broadcast")]
    Broadcast {
        message_id: Uuid,
        sender_id: Option<Uuid>,
        sender: String,
        sender_avatar: String,
        content: String,
        attachment: Option<Attachment>,
        reply_to: Option<ReplyPreview>,
        recalled_at: Option<DateTime<Utc>>,
        edited_at: Option<DateTime<Utc>>,
        timestamp: DateTime<Utc>,
    },

    /// Server -> Client: a sender replaced a message's content.
    #[serde(rename = "message_edited")]
    MessageEdited {
        message_id: Uuid,
        content: String,
        edited_at: DateTime<Utc>,
    },

    /// Server -> Client: a message was marked as recalled.
    #[serde(rename = "message_recalled")]
    MessageRecalled {
        message_id: Uuid,
        recalled_at: DateTime<Utc>,
    },

    /// Server -> Client: the room's current unique member snapshot changed.
    #[serde(rename = "presence")]
    Presence {
        members: Vec<RoomMember>,
        participants: Vec<RoomMember>,
    },

    /// Server -> Client: a participant advanced their read position.
    #[serde(rename = "read_receipt")]
    ReadReceipt {
        user_id: Uuid,
        username: String,
        message_id: Uuid,
    },

    /// Server → Client: system event (join / leave).
    #[serde(rename = "system")]
    System {
        content: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        members: Option<Vec<RoomMember>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        participants: Option<Vec<RoomMember>>,
    },
}
