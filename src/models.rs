//! Data models — serialisable structs shared across REST and WebSocket.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

/// The chat-domain types live in `crate::chats::models`, beside the module that owns them
/// (`AGENTS.md`), and are re-exported here so existing `crate::models::…` imports keep working.
pub use crate::chats::models::{
    Chat, ChatCompatView, ChatMember, ChatMembership, CreateChatRequest, InviteMemberRequest,
    JoinChatRequest, UpdateChatRequest, UpdateMembershipRequest, UpdateNicknameRequest,
};

// ── REST models ──────────────────────────────────────────────────────────────

/// Point-in-time snapshot of a message's original sender/chat, kept even if the
/// source message or chat is later recalled, edited, or soft-deleted.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ForwardedFrom {
    pub sender: String,
    pub room_name: String,
}

/// A chat message persisted as part of a chat session.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct StoredMessage {
    pub id: Uuid,
    pub client_message_id: Option<Uuid>,
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
    #[serde(default)]
    pub favorite_id: Option<Uuid>,
    pub forwarded_from: Option<ForwardedFrom>,
    #[serde(default)]
    pub reactions: Vec<MessageReaction>,
}

/// Aggregated users who applied one emoji response to a message.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct MessageReaction {
    pub emoji: String,
    pub user_ids: Vec<Uuid>,
}

/// Metadata needed to display or download a message attachment.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Attachment {
    pub id: Uuid,
    pub file_name: String,
    pub mime_type: String,
    pub size_bytes: i64,
    pub download_url: String,
    pub is_sensitive: bool,
}

/// One attachment-bearing message returned by the paginated chat file browser.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ChatFileItem {
    pub message_id: Uuid,
    pub sender_id: Option<Uuid>,
    pub sender: String,
    pub sender_avatar: String,
    pub created_at: DateTime<Utc>,
    pub attachment: Attachment,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ChatFilePage {
    pub items: Vec<ChatFileItem>,
    pub next_before: Option<Uuid>,
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

/// The newest message a chat participant has viewed.
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
    pub display_name: String,
    pub signature: String,
    pub homepage: String,
    pub created_at: DateTime<Utc>,
}

/// Compact public account data embedded in relationship and conversation views.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
pub struct UserSummary {
    pub id: Uuid,
    pub username: String,
    pub avatar_emoji: String,
    pub display_name: String,
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
    pub avatar_emoji: Option<String>,
    pub display_name: Option<String>,
    pub signature: Option<String>,
    pub homepage: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct DeleteAccountRequest {
    pub current_password: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct VerifyPasswordRequest {
    pub current_password: String,
}

/// A login session returned after successful registration or authentication.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AuthSession {
    pub token: Uuid,
    pub user: User,
    pub expires_at: DateTime<Utc>,
}

/// Payload for POST /api/messages/forward.
#[derive(Debug, Deserialize, ToSchema)]
pub struct ForwardMessagesRequest {
    pub message_ids: Vec<Uuid>,
    pub target_room_ids: Vec<Uuid>,
}

/// One (source message, target chat) forward outcome.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ForwardResult {
    pub message_id: Uuid,
    pub target_room_id: Uuid,
    pub forwarded_message_id: Option<Uuid>,
    pub skipped_reason: Option<String>,
}

// ── WebSocket message envelope ───────────────────────────────────────────────

/// Every WebSocket frame carries one JSON-serialised ChatMessage.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
// This transport enum intentionally mirrors the JSON protocol without boxed wire fields.
#[allow(clippy::large_enum_variant)]
pub enum ChatMessage {
    /// Client → Server: join a public chat (no password needed).
    #[serde(rename = "join")]
    Join { token: Uuid },

    /// Client → Server: authenticate with chat password.
    #[serde(rename = "auth")]
    Auth { token: Uuid, password: String },

    /// Server → Client: authentication / join succeeded.
    #[serde(rename = "auth_ok")]
    AuthOk {
        room_name: String,
        members: Vec<ChatMember>,
        participants: Vec<ChatMember>,
        read_receipts: Vec<ReadReceipt>,
    },

    /// Server -> Client: all persisted history for this connection was replayed.
    #[serde(rename = "history_complete")]
    HistoryComplete,

    /// Server → Client: authentication / join failed.
    #[serde(rename = "auth_fail")]
    AuthFail { reason: String },

    /// Client → Server: send a chat message.
    #[serde(rename = "message")]
    Message {
        content: String,
        #[serde(default)]
        reply_to: Option<Uuid>,
        #[serde(default)]
        client_message_id: Option<Uuid>,
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

    /// Client -> Server: advance this account's read position in the chat.
    #[serde(rename = "read")]
    Read { message_id: Uuid },

    /// Client -> Server: recall a message sent by this account.
    #[serde(rename = "recall")]
    Recall { message_id: Uuid },

    /// Client -> Server: explicitly add or remove one emoji response.
    #[serde(rename = "reaction")]
    Reaction {
        message_id: Uuid,
        emoji: String,
        active: bool,
    },

    /// Client -> Server: nudge another connected member in this chat.
    #[serde(rename = "poke")]
    Poke { target_user_id: Uuid },

    /// Server → Client: a chat message broadcast from another user.
    #[serde(rename = "broadcast")]
    Broadcast {
        message_id: Uuid,
        #[serde(skip_serializing_if = "Option::is_none")]
        client_message_id: Option<Uuid>,
        sender_id: Option<Uuid>,
        sender: String,
        sender_avatar: String,
        content: String,
        attachment: Option<Attachment>,
        reply_to: Option<ReplyPreview>,
        recalled_at: Option<DateTime<Utc>>,
        edited_at: Option<DateTime<Utc>>,
        timestamp: DateTime<Utc>,
        #[serde(default)]
        favorite_id: Option<Uuid>,
        forwarded_from: Option<ForwardedFrom>,
        #[serde(default)]
        reactions: Vec<MessageReaction>,
    },

    /// Server -> Client: one member added or removed an emoji response.
    #[serde(rename = "reaction_changed")]
    ReactionChanged {
        message_id: Uuid,
        emoji: String,
        user_id: Uuid,
        active: bool,
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

    /// Server -> Client: the chat's current unique member snapshot changed.
    #[serde(rename = "presence")]
    Presence {
        members: Vec<ChatMember>,
        participants: Vec<ChatMember>,
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
        members: Option<Vec<ChatMember>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        participants: Option<Vec<ChatMember>>,
    },
}
