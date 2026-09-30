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

/// The WebSocket protocol lives in `crate::realtime` (TG-007), which owns the frames, and is
/// re-exported here for the same reason.
pub use crate::realtime::frames::ChatMessage;
pub use crate::realtime::payloads::{
    MessageViewCount, PollOption, PollState, TopicSummary, TypingAction, UserStatus,
    UserStatusEntry,
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
#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
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
    /// TG-302: `"sticker"` for sticker messages; absent for text and plain attachments.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media_kind: Option<String>,
    /// TG-302: which sticker a sticker message sent; the file is `attachment`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sticker: Option<crate::stickers::models::MessageSticker>,
    /// TG-304: formatted ranges of `content` (custom emoji); omitted when there are none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entities: Vec<crate::stickers::custom_emoji::MessageEntity>,
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

// The WebSocket message envelope (`ChatMessage`) lives in `crate::realtime::frames` since
// TG-007 and is re-exported at the top of this file.
