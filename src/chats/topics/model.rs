//! Wire types and pure rules of forum topics (`docs/devlog/TG-204.md`, Frozen interface).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::realtime::payloads::TopicSummary;

/// Telegram's six topic icon colours, in its own order.
pub const TOPIC_COLORS: [i64; 6] = [0x6FB9F0, 0xFFD67E, 0xCB86DB, 0x8EEE98, 0xFF93B2, 0xFB6F5F];
pub const MAX_TOPIC_TITLE_CHARS: usize = 128;
pub const MAX_TOPIC_ICON_EMOJI_CHARS: usize = 16;
/// The title a forum's General topic starts with (Telegram's is "General").
pub const GENERAL_TOPIC_TITLE: &str = "General";

/// A topic's default colour: stable for the topic, spread over the palette.
pub fn default_topic_color(id: Uuid) -> i64 {
    TOPIC_COLORS[usize::from(id.as_bytes()[15]) % TOPIC_COLORS.len()]
}

/// The newest message of a topic, as its row in the topic list previews it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct TopicLastMessage {
    pub message_id: Uuid,
    pub sender: String,
    /// Empty when the message was recalled.
    pub content: String,
    pub created_at: DateTime<Utc>,
}

/// One topic as a viewer sees it: `unread_count`, `muted*` and `can_edit` are the viewer's.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct ForumTopic {
    pub id: Uuid,
    pub chat_id: Uuid,
    pub is_general: bool,
    pub title: String,
    pub icon_emoji: String,
    pub icon_custom_emoji_id: Option<Uuid>,
    pub icon_color: i64,
    pub is_pinned: bool,
    pub pinned_at: Option<DateTime<Utc>>,
    pub is_closed: bool,
    pub is_hidden: bool,
    pub creator_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub last_message: Option<TopicLastMessage>,
    pub unread_count: i64,
    pub muted: bool,
    pub muted_until: Option<DateTime<Utc>>,
    pub can_edit: bool,
}

impl ForumTopic {
    /// Pinned first (in pin order), then General, then the most recently active.
    pub fn sort_key(&self) -> (u8, i64) {
        match (self.pinned_at, self.is_general) {
            (Some(pinned_at), _) => (0, pinned_at.timestamp_micros()),
            (None, true) => (1, 0),
            (None, false) => {
                let active = self
                    .last_message
                    .as_ref()
                    .map_or(self.created_at, |last| last.created_at);
                (2, -active.timestamp_micros())
            }
        }
    }

    pub fn summary(&self) -> TopicSummary {
        TopicSummary {
            id: self.id,
            title: self.title.clone(),
            icon_emoji: self.icon_emoji.clone(),
            closed: self.is_closed,
            pinned: self.is_pinned,
            icon_color: Some(self.icon_color),
            icon_custom_emoji_id: self.icon_custom_emoji_id,
            hidden: self.is_hidden,
            is_general: self.is_general,
            deleted: false,
        }
    }
}

/// `GET /api/chats/:id/topics`.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ForumTopicList {
    pub chat_id: Uuid,
    pub is_forum: bool,
    /// The viewer may create topics (`chat.topics`).
    pub can_create: bool,
    /// The viewer is a topic administrator: may pin, hide, delete, edit any topic and post
    /// in closed ones.
    pub can_manage: bool,
    pub topics: Vec<ForumTopic>,
}

/// `POST /api/chats/:id/topics`.
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct CreateTopicRequest {
    pub title: String,
    #[serde(default)]
    pub icon_emoji: Option<String>,
    #[serde(default)]
    pub icon_custom_emoji_id: Option<Uuid>,
    #[serde(default)]
    pub icon_color: Option<i64>,
}

/// `PATCH /api/chats/:id/topics/:topic_id`. Absent fields stay; `icon_custom_emoji_id: null`
/// clears the custom emoji.
#[derive(Debug, Clone, Default, Deserialize, ToSchema)]
pub struct UpdateTopicRequest {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub icon_emoji: Option<String>,
    #[serde(default, deserialize_with = "double_option")]
    #[schema(value_type = Option<Uuid>)]
    pub icon_custom_emoji_id: Option<Option<Uuid>>,
    #[serde(default)]
    pub icon_color: Option<i64>,
    #[serde(default)]
    pub is_closed: Option<bool>,
    #[serde(default)]
    pub is_pinned: Option<bool>,
    #[serde(default)]
    pub is_hidden: Option<bool>,
}

impl UpdateTopicRequest {
    /// Fields only a topic administrator may change (the creator may not).
    pub fn needs_manager(&self) -> bool {
        self.is_pinned.is_some() || self.is_hidden.is_some()
    }
}

fn double_option<'de, D>(deserializer: D) -> Result<Option<Option<Uuid>>, D::Error>
where
    D: Deserializer<'de>,
{
    Option::<Uuid>::deserialize(deserializer).map(Some)
}

/// `POST /api/chats/:id/topics/:topic_id/read`.
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct TopicReadRequest {
    pub message_id: Uuid,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct TopicReadResult {
    pub topic_id: Uuid,
    pub unread_count: i64,
    /// The read left no unread message in any topic, so the chat-level cursor followed.
    pub chat_read_advanced: bool,
}

/// `PUT /api/chats/:id/topics/:topic_id/notifications`.
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct TopicNotificationsRequest {
    pub muted: bool,
    #[serde(default)]
    pub muted_until: Option<DateTime<Utc>>,
}

/// `PUT /api/chats/:id/forum`.
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct ForumToggleRequest {
    pub enabled: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TopicHistoryQuery {
    pub limit: Option<i64>,
    pub before: Option<Uuid>,
}

/// Why a topic operation was refused. Handlers map this to a status code.
#[derive(Debug)]
pub enum TopicError {
    /// The chat is not a forum (or cannot be one).
    NotForum,
    NotFound,
    Forbidden,
    /// Posting into a closed topic without being a topic administrator.
    Closed,
    Invalid(&'static str),
    /// General cannot be deleted.
    Conflict(&'static str),
    /// TG-207: slow mode — the sender must wait this many more seconds.
    SlowMode(i64),
    Database(sqlx::Error),
}

impl From<sqlx::Error> for TopicError {
    fn from(error: sqlx::Error) -> Self {
        Self::Database(error)
    }
}

/// A validated title: trimmed, 1..=128 characters.
pub fn valid_title(title: &str) -> Result<String, TopicError> {
    let title = title.trim();
    let length = title.chars().count();
    if length == 0 || length > MAX_TOPIC_TITLE_CHARS {
        return Err(TopicError::Invalid("title must be 1-128 characters"));
    }
    Ok(title.to_string())
}

pub fn valid_icon_emoji(emoji: &str) -> Result<String, TopicError> {
    let emoji = emoji.trim();
    if emoji.chars().count() > MAX_TOPIC_ICON_EMOJI_CHARS {
        return Err(TopicError::Invalid("icon_emoji is too long"));
    }
    Ok(emoji.to_string())
}

pub fn valid_color(color: i64) -> Result<i64, TopicError> {
    TOPIC_COLORS
        .contains(&color)
        .then_some(color)
        .ok_or(TopicError::Invalid(
            "icon_color must be one of the six topic colours",
        ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles_are_trimmed_and_bounded() {
        assert_eq!(valid_title("  Ideas ").unwrap(), "Ideas");
        assert!(valid_title("   ").is_err());
        assert!(valid_title(&"話".repeat(128)).is_ok());
        assert!(valid_title(&"話".repeat(129)).is_err());
    }

    #[test]
    fn colours_come_from_the_palette() {
        assert!(valid_color(0x6FB9F0).is_ok());
        assert!(valid_color(0x123456).is_err());
        for _ in 0..32 {
            assert!(TOPIC_COLORS.contains(&default_topic_color(Uuid::new_v4())));
        }
    }

    #[test]
    fn update_clears_custom_emoji_only_when_explicit() {
        let absent: UpdateTopicRequest = serde_json::from_str("{}").unwrap();
        assert_eq!(absent.icon_custom_emoji_id, None);
        let cleared: UpdateTopicRequest =
            serde_json::from_str(r#"{"icon_custom_emoji_id":null}"#).unwrap();
        assert_eq!(cleared.icon_custom_emoji_id, Some(None));
        assert!(!cleared.needs_manager());
        let pin: UpdateTopicRequest = serde_json::from_str(r#"{"is_pinned":true}"#).unwrap();
        assert!(pin.needs_manager());
    }
}
