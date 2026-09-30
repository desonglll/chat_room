//! The WebSocket frame envelope.
//!
//! Moved here from `src/models.rs` by TG-007 (the realtime module owns the wire protocol);
//! `crate::models` re-exports [`ChatMessage`] so existing imports keep working. The frame
//! shapes are frozen in `docs/devlog/TG-007.md`: TG-007 was the one deliberate breaking
//! window, and from here on the protocol only grows — new frames or new optional fields,
//! never a changed or removed shape. Serialisation is pinned byte-for-byte by
//! `tests/ws_frame_snapshot_legacy_test.rs` and `tests/ws_frame_snapshot_extension_test.rs`.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::models::{
    Attachment, Chat, ChatMember, ChatMembership, ForwardedFrom, MessageReaction, ReadReceipt,
    ReplyPreview,
};
use crate::realtime::payloads::{
    MessageViewCount, PollState, TopicSummary, TypingAction, UserStatus, UserStatusEntry,
};
use crate::stickers::custom_emoji::MessageEntity;
use crate::stickers::models::MessageSticker;

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
    ///
    /// `room_name` is the pre-TG-004 spelling the frozen clients read; it stays (CONTEXT.md,
    /// "Room"). `statuses` carries one entry per active participant: `online` for currently
    /// connected accounts, `empty` for the rest until TG-505 adds persisted last-seen data.
    #[serde(rename = "auth_ok")]
    AuthOk {
        room_name: String,
        members: Vec<ChatMember>,
        participants: Vec<ChatMember>,
        read_receipts: Vec<ReadReceipt>,
        #[serde(default)]
        statuses: Vec<UserStatusEntry>,
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
        /// TG-304: optional formatted ranges of `content` (docs/devlog/TG-304.md).
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        entities: Vec<MessageEntity>,
    },

    /// Client -> Server: replace the content of a message sent by this account. TG-304: the
    /// optional `entities` replace the message's entities (absent = none).
    #[serde(rename = "edit")]
    Edit {
        message_id: Uuid,
        content: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        entities: Vec<MessageEntity>,
    },

    /// Both directions: publish a transient draft and what the sender is doing.
    ///
    /// `action` is optional on input (absent means `typing`, which is what every pre-TG-007
    /// client sends) and always present on output. Empty `content` with `action: "typing"`
    /// keeps its legacy meaning of "stopped"; server-initiated clears use `cancel`.
    #[serde(rename = "typing")]
    Typing {
        content: String,
        #[serde(default)]
        action: TypingAction,
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
        /// TG-302: optional, omitted unless the message is a sticker (docs/devlog/TG-302.md).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        media_kind: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        sticker: Option<MessageSticker>,
        /// TG-304: omitted unless the message has entities.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        entities: Vec<MessageEntity>,
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
        /// TG-304: the edited text's entities; omitted (= none) when empty.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        entities: Vec<MessageEntity>,
    },

    /// Server -> Client: a message was marked as recalled.
    #[serde(rename = "message_recalled")]
    MessageRecalled {
        message_id: Uuid,
        recalled_at: DateTime<Utc>,
    },

    /// Server -> Client: the chat's current unique member snapshot changed.
    ///
    /// Unchanged by TG-007: per-user status travels in `user_status` frames, not here, and
    /// clients must not assume any relative order between the two kinds.
    #[serde(rename = "presence")]
    Presence {
        members: Vec<ChatMember>,
        participants: Vec<ChatMember>,
    },

    /// Server -> Client: one account's presence changed (TG-007).
    ///
    /// Emitted live on the first connection (`online`) and last disconnection (`offline`
    /// with `last_seen = now`) of an account. The privacy-tier statuses are TG-505's.
    #[serde(rename = "user_status")]
    UserStatusChanged { user_id: Uuid, status: UserStatus },

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

    // ── TG-007 frame skeletons ──────────────────────────────────────────────
    // Shape and broadcast path only; the emitting business logic lands in later milestones
    // (docs/devlog/TG-007.md, Frozen interface §3). All server → client; the server ignores
    // any of them arriving from a client.
    /// Server -> Client: the chat descriptor changed (title, avatar, permissions, slow mode…).
    /// Carries the same descriptor JSON as `GET /api/chats/:id`. Filled by M2.
    #[serde(rename = "chat_updated")]
    ChatUpdated { chat: Chat },

    /// Server -> Client: one membership changed (join / leave / role / restriction). M2.
    #[serde(rename = "member_updated")]
    MemberUpdated { member: ChatMembership },

    /// Server -> Client: a forum topic was created, closed, or (un)pinned. M2 (TG-204).
    #[serde(rename = "topic_updated")]
    TopicUpdated { topic: TopicSummary },

    /// Server -> Client: batched channel view counters. One frame, many message ids. M2.
    #[serde(rename = "message_views_updated")]
    MessageViewsUpdated { views: Vec<MessageViewCount> },

    /// Server -> Client: poll results changed. M4.
    #[serde(rename = "poll_updated")]
    PollUpdated { message_id: Uuid, poll: PollState },

    /// Server -> Client: this account's cloud draft changed on another connection (TG-008).
    ///
    /// Broadcast on the chat channel; the transport (`frame_visible_to` in
    /// `src/realtime/protocol.rs`) delivers it only to `user_id`'s own connections.
    #[serde(rename = "draft_updated")]
    DraftUpdated {
        user_id: Uuid,
        text: String,
        reply_to_message_id: Option<Uuid>,
        topic_id: Option<Uuid>,
        updated_at: DateTime<Utc>,
    },
}
