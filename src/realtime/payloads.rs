//! Wire payload types embedded in [`crate::models::ChatMessage`] frames.
//!
//! TG-007 froze these shapes (`docs/devlog/TG-007.md`, "Frozen interface"): later milestones
//! extend them additively — new optional fields, new enum values — but never rename or remove
//! what is here.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

/// What a member is currently doing in the chat, carried by the `typing` frame.
///
/// Optional on input (a pre-TG-007 client sends none, which means [`TypingAction::Typing`]);
/// always present on output. An unknown string deserialises to `Typing` so a newer client's
/// granular state degrades to a plain typing indicator instead of a dropped frame — client
/// implementations must mirror that rule.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", from = "String")]
pub enum TypingAction {
    #[default]
    Typing,
    RecordingVoice,
    RecordingVideoNote,
    UploadingPhoto,
    UploadingVideo,
    UploadingDocument,
    UploadingVoice,
    ChoosingSticker,
    ChoosingLocation,
    /// Explicit "stopped" state. Legacy clients signal the same thing with empty `content`.
    Cancel,
}

impl From<String> for TypingAction {
    fn from(value: String) -> Self {
        match value.as_str() {
            "recording_voice" => Self::RecordingVoice,
            "recording_video_note" => Self::RecordingVideoNote,
            "uploading_photo" => Self::UploadingPhoto,
            "uploading_video" => Self::UploadingVideo,
            "uploading_document" => Self::UploadingDocument,
            "uploading_voice" => Self::UploadingVoice,
            "choosing_sticker" => Self::ChoosingSticker,
            "choosing_location" => Self::ChoosingLocation,
            "cancel" => Self::Cancel,
            // "typing" and every action this server does not know yet.
            _ => Self::Typing,
        }
    }
}

/// Per-user presence for the `user_status` frame and `auth_ok.statuses`.
///
/// `Recently` / `WithinWeek` / `WithinMonth` / `LongAgo` are the privacy tiers — Telegram's
/// deliberately obscured statuses. They are placeholders until TG-505 implements the real
/// per-viewer privacy rules; nothing emits them yet. `Empty` means "no information".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum UserStatus {
    Online,
    Offline { last_seen: DateTime<Utc> },
    Recently,
    WithinWeek,
    WithinMonth,
    LongAgo,
    Empty,
}

/// One `(user, status)` pair inside `auth_ok.statuses`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserStatusEntry {
    pub user_id: Uuid,
    pub status: UserStatus,
}

/// Forum-topic snapshot carried by `topic_updated` (TG-204, `docs/devlog/TG-204.md`).
///
/// TG-007 froze `id`/`title`/`icon_emoji`/`closed`/`pinned`; TG-204 grew it **additively**:
/// every later field is omitted at its default, so the TG-007 snapshot is byte-for-byte stable.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TopicSummary {
    pub id: Uuid,
    pub title: String,
    pub icon_emoji: String,
    pub closed: bool,
    pub pinned: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon_color: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon_custom_emoji_id: Option<Uuid>,
    /// General only: hidden from the topic list.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub hidden: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub is_general: bool,
    /// The topic and its messages were deleted.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub deleted: bool,
}

/// One entry of the **batched** `message_views_updated` frame. One frame carries many ids;
/// per-message frames would flood every channel subscriber (architecture.md §5.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessageViewCount {
    pub message_id: Uuid,
    pub views: i64,
}

/// Poll snapshot carried by `poll_updated`, and by a poll message's `poll` field.
///
/// TG-007 froze `id`/`question`/`closed`/`total_voters`/`options`; TG-406 filled the logic and
/// grew the shape **additively**: every later field is omitted at its default, so the TG-007
/// snapshot serialises byte-for-byte as before. `id` is the carrying message's id (a poll is
/// keyed by its message). See `docs/devlog/TG-406.md`, Frozen interface.
///
/// `poll_updated` is chat-wide, so it never carries viewer-specific data: `chosen` is absent
/// (`None` = "not known in this frame", merge-keep the local value), and a quiz's
/// `correct_option`/`explanation` appear only once the poll is closed. A per-viewer read
/// (history, `GET /api/polls/:id`, a vote response) sets `chosen` (`Some([])` = not voted) and
/// reveals the quiz answer to a viewer who has answered. No frame ever carries voter ids.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct PollState {
    pub id: Uuid,
    pub question: String,
    pub closed: bool,
    pub total_voters: i64,
    pub options: Vec<PollOption>,
    /// Voters are visible (`GET /api/polls/:id/voters`). Absent means anonymous.
    #[serde(default, skip_serializing_if = "is_false")]
    pub public_voters: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub multiple_choice: bool,
    /// Quiz mode: exactly one correct option, and a vote can never be changed or retracted.
    #[serde(default, skip_serializing_if = "is_false")]
    pub quiz: bool,
    /// Index into `options`. Quiz only, and only when revealed (see type docs).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub correct_option: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub explanation: Option<String>,
    /// The viewer's own chosen option indexes. Absent in chat-wide frames.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chosen: Option<Vec<u32>>,
    /// Monotonic per poll; a client drops any snapshot older than the one it holds.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub revision: i64,
}

/// One answer row of a [`PollState`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct PollOption {
    pub text: String,
    pub voters: i64,
}

fn is_false(value: &bool) -> bool {
    !*value
}

fn is_zero(value: &i64) -> bool {
    *value == 0
}
