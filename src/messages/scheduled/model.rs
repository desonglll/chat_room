//! Wire types, limits and errors of the scheduled-message interface (TG-404).

use axum::http::StatusCode;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::stickers::custom_emoji::MessageEntity;

/// Telegram keeps at most 100 scheduled messages per chat and sender.
pub const MAX_SCHEDULED_PER_CHAT: i64 = 100;
/// Telegram refuses dates further than a year ahead.
pub const MAX_SCHEDULE_AHEAD_DAYS: i64 = 366;

/// One pending scheduled message, visible only to its author. `id` becomes the delivered
/// message's id, so a client can match the `broadcast` frame that replaces this entry.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ScheduledMessage {
    pub id: Uuid,
    pub chat_id: Uuid,
    pub content: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entities: Vec<MessageEntity>,
    pub reply_to: Option<Uuid>,
    pub silent: bool,
    /// TG-204: the forum topic it will be delivered into; omitted for General.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub topic_id: Option<Uuid>,
    pub scheduled_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateScheduledMessageRequest {
    pub content: String,
    #[serde(default)]
    pub entities: Vec<MessageEntity>,
    #[serde(default)]
    pub reply_to: Option<Uuid>,
    pub scheduled_at: DateTime<Utc>,
    #[serde(default)]
    pub silent: bool,
    /// TG-204: the forum topic to deliver into; absent = General.
    #[serde(default)]
    pub topic_id: Option<Uuid>,
}

/// Every field is optional; absent fields keep their value. `entities` replaces the entities
/// only together with `content` (a new text invalidates the old ranges).
#[derive(Debug, Default, Deserialize, ToSchema)]
pub struct UpdateScheduledMessageRequest {
    pub content: Option<String>,
    pub entities: Option<Vec<MessageEntity>>,
    pub scheduled_at: Option<DateTime<Utc>>,
    pub silent: Option<bool>,
}

#[derive(Debug)]
pub enum ScheduledError {
    /// No such chat / scheduled message, or the caller is not its member / author.
    NotFound,
    /// The caller may not send messages in this chat.
    Forbidden,
    /// Empty or oversize text, or a date in the past or too far ahead.
    Invalid,
    /// The per-chat limit of pending scheduled messages is reached.
    Limit,
    /// The message write queue is saturated; retry.
    Busy,
    Database(sqlx::Error),
}

impl ScheduledError {
    /// TG-204's topic gate, in this module's wire vocabulary.
    pub(crate) fn from_topic(error: crate::chats::TopicError) -> Self {
        use crate::chats::TopicError;
        match error {
            TopicError::Database(error) => Self::Database(error),
            TopicError::NotFound => Self::NotFound,
            TopicError::Closed | TopicError::Forbidden => Self::Forbidden,
            _ => Self::Invalid,
        }
    }
}

impl From<sqlx::Error> for ScheduledError {
    fn from(error: sqlx::Error) -> Self {
        Self::Database(error)
    }
}

impl ScheduledError {
    pub fn status(&self) -> StatusCode {
        match self {
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::Forbidden => StatusCode::FORBIDDEN,
            Self::Invalid => StatusCode::BAD_REQUEST,
            Self::Limit => StatusCode::CONFLICT,
            Self::Busy => StatusCode::SERVICE_UNAVAILABLE,
            Self::Database(error) => {
                tracing::error!("scheduled message storage failed: {error}");
                StatusCode::INTERNAL_SERVER_ERROR
            }
        }
    }
}

/// A delivery date must lie in the future and within [`MAX_SCHEDULE_AHEAD_DAYS`].
pub(crate) fn validate_date(scheduled_at: DateTime<Utc>, now: DateTime<Utc>) -> bool {
    scheduled_at > now && scheduled_at <= now + Duration::days(MAX_SCHEDULE_AHEAD_DAYS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_must_be_future_and_within_a_year() {
        let now = Utc::now();
        assert!(!validate_date(now, now));
        assert!(!validate_date(now - Duration::seconds(1), now));
        assert!(validate_date(now + Duration::seconds(1), now));
        assert!(validate_date(
            now + Duration::days(MAX_SCHEDULE_AHEAD_DAYS),
            now
        ));
        assert!(!validate_date(
            now + Duration::days(MAX_SCHEDULE_AHEAD_DAYS) + Duration::seconds(1),
            now
        ));
    }
}
