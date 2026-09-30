//! Chat-domain request and response types.
//!
//! `AGENTS.md`: domain request/response types live beside the domain implementation, not in
//! `src/models.rs`. `crate::models` re-exports them so that the ~40 call sites that predate
//! TG-005 keep their `use crate::models::{Chat, ...}` imports.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use super::ChatType;

/// Public-facing chat descriptor (password hash and access hash are never serialised).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
pub struct Chat {
    pub id: Uuid,
    /// Which of the four conversation shapes this is. See `chats::ChatType`.
    #[serde(default)]
    #[sqlx(try_from = "String")]
    pub chat_type: ChatType,
    /// Display title. Stored in `chats.title`; the pre-TG-006 wire name was `name` and
    /// `ChatCompatView` still emits it for the deprecated `/api/rooms/*` alias.
    #[serde(alias = "name")]
    pub title: String,
    /// SHA-256 hex digest — empty string means the chat is public.
    #[serde(skip_serializing, default)]
    #[schema(value_type = String)]
    pub password_hash: String,
    /// Whether a password is required to join.
    pub has_password: bool,
    pub creator_user_id: Option<Uuid>,
    pub join_policy: String,
    #[serde(default)]
    #[sqlx(default)]
    pub avatar_emoji: String,
    #[serde(default)]
    #[sqlx(default)]
    pub description: String,
    /// Public `@handle`. Only a supergroup or a channel may hold one, and it is unique across
    /// every chat that is not soft-deleted (`chats_username_active_idx`).
    #[serde(default)]
    #[sqlx(default)]
    pub username: Option<String>,
    /// 64 bits that let a public handle be resolved without exposing the row id. Never
    /// serialised in M0; M2 decides whether public resolution hands it to clients.
    #[serde(skip_serializing, default)]
    #[schema(value_type = String)]
    #[sqlx(default)]
    pub access_hash: String,
    #[serde(default)]
    #[sqlx(default)]
    pub is_forum: bool,
    /// A channel's discussion group, which is where its comment threads live.
    #[serde(default)]
    #[sqlx(default)]
    pub linked_chat_id: Option<Uuid>,
    #[serde(default)]
    #[sqlx(default)]
    pub slow_mode_seconds: i64,
    #[serde(default)]
    #[sqlx(default)]
    pub auto_delete_seconds: i64,
    #[serde(default)]
    #[sqlx(default)]
    pub signatures_enabled: bool,
    #[serde(default)]
    #[sqlx(default)]
    pub history_visible_to_new_members: bool,
    /// A projection maintained by the transaction that changes membership. The authoritative
    /// count is still `COUNT(*)` over `chat_members`, which a 200 000-member supergroup cannot
    /// afford per request.
    #[serde(default)]
    #[sqlx(default)]
    pub member_count: i64,
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

impl Default for Chat {
    /// A blank private-nothing placeholder. It exists so that the handful of call sites that
    /// build a `Chat` from a different table (`direct_conversations`, `conversations`) can
    /// spell out the fields they know and inherit the rest, instead of each repeating eleven
    /// M2 defaults that would then drift apart. `create_chat` sets every field explicitly.
    fn default() -> Self {
        Self {
            id: Uuid::nil(),
            chat_type: ChatType::Group,
            title: String::new(),
            password_hash: String::new(),
            has_password: false,
            creator_user_id: None,
            join_policy: "open".into(),
            avatar_emoji: String::new(),
            description: String::new(),
            username: None,
            access_hash: String::new(),
            is_forum: false,
            linked_chat_id: None,
            slow_mode_seconds: 0,
            auto_delete_seconds: 0,
            signatures_enabled: false,
            history_visible_to_new_members: true,
            member_count: 0,
            membership_status: None,
            membership_role: None,
            unread_count: 0,
            created_at: DateTime::UNIX_EPOCH,
        }
    }
}

/// The chat descriptor as the pre-TG-006 clients expect it: every field of [`Chat`] plus the
/// deprecated `name` duplicate of `title`.
///
/// Served by the `/api/rooms/*` alias and by `/api/conversations`, which has no alias of its
/// own and is still read by the frozen Vue, PySide6 and ratatui clients. Delete this struct in
/// M6 together with those clients; nothing else has to change when it goes.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ChatCompatView {
    #[serde(flatten)]
    pub chat: Chat,
    /// Deprecated pre-TG-006 spelling of `title`.
    #[schema(deprecated)]
    pub name: String,
}

impl From<Chat> for ChatCompatView {
    fn from(chat: Chat) -> Self {
        Self {
            name: chat.title.clone(),
            chat,
        }
    }
}

/// A unique signed-in account currently connected to a chat.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMember {
    pub user_id: Uuid,
    pub username: String,
    pub avatar_emoji: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
pub struct ChatMembership {
    pub user_id: Uuid,
    pub username: String,
    pub avatar_emoji: String,
    pub nickname: String,
    pub role: String,
    pub status: String,
    pub requested_at: DateTime<Utc>,
    pub joined_at: Option<DateTime<Utc>>,
}

/// Payload for POST /api/chats.
///
/// `title` is also accepted under its pre-TG-006 name `name`, on both the canonical path and
/// the deprecated alias, so neither a pre-rename nor a post-rename client has to care which
/// path it posts to.
#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateChatRequest {
    #[serde(alias = "name")]
    pub title: String,
    /// Plain-text password — omit or set to "" for a public chat.
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default)]
    pub join_policy: Option<String>,
    #[serde(default)]
    pub avatar_emoji: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

/// Payload for PATCH /api/chats/{id}. Missing fields remain unchanged.
///
/// `title` is also accepted under its pre-TG-006 name `name`; see [`CreateChatRequest`].
#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateChatRequest {
    #[serde(default, alias = "name")]
    pub title: Option<String>,
    /// Required for every change to a private chat.
    #[serde(default)]
    pub current_password: Option<String>,
    /// Set to an empty string to make the chat public.
    pub new_password: Option<String>,
    pub join_policy: Option<String>,
    #[serde(default)]
    pub avatar_emoji: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

/// Payload for PATCH /api/chats/{id}/members/me (self-service nickname).
#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateNicknameRequest {
    pub nickname: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct JoinChatRequest {
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
