//! Wire payload types embedded in [`crate::models::ChatMessage`] frames.
//!
//! TG-007 froze these shapes (`docs/devlog/TG-007.md`, "Frozen interface"): later milestones
//! extend them additively — new optional fields, new enum values — but never rename or remove
//! what is here.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
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

/// Forum-topic snapshot carried by `topic_updated`. Business logic lands in M2 (TG-204).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TopicSummary {
    pub id: Uuid,
    pub title: String,
    pub icon_emoji: String,
    pub closed: bool,
    pub pinned: bool,
}

/// One entry of the **batched** `message_views_updated` frame. One frame carries many ids;
/// per-message frames would flood every channel subscriber (architecture.md §5.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessageViewCount {
    pub message_id: Uuid,
    pub views: i64,
}

/// Poll snapshot carried by `poll_updated`. Business logic lands in M4.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PollState {
    pub id: Uuid,
    pub question: String,
    pub closed: bool,
    pub total_voters: i64,
    pub options: Vec<PollOption>,
}

/// One answer row of a [`PollState`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PollOption {
    pub text: String,
    pub voters: i64,
}
