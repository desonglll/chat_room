//! Persistence of `forum_topics`: the General row, creation, edits and deletion.

use chrono::{DateTime, Utc};
use sqlx::FromRow;
use uuid::Uuid;

use super::model::{
    default_topic_color, valid_color, valid_icon_emoji, valid_title, TopicError,
    UpdateTopicRequest, GENERAL_TOPIC_TITLE,
};
use crate::realtime::payloads::TopicSummary;
use crate::state::{with_pool, AppState};

pub(crate) const TOPIC_SELECT: &str = "SELECT id, room_id, is_general, title, icon_emoji, \
    icon_custom_emoji_id, CAST(icon_color AS BIGINT) AS icon_color, pinned_at, closed_at, \
    is_hidden, creator_id, created_at FROM forum_topics";

#[derive(Debug, Clone, FromRow)]
pub(crate) struct TopicRow {
    pub id: Uuid,
    pub room_id: Uuid,
    pub is_general: bool,
    pub title: String,
    pub icon_emoji: String,
    pub icon_custom_emoji_id: Option<Uuid>,
    pub icon_color: i64,
    pub pinned_at: Option<DateTime<Utc>>,
    pub closed_at: Option<DateTime<Utc>>,
    pub is_hidden: bool,
    pub creator_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

impl TopicRow {
    /// The value `messages.topic_id` holds for this topic: `NULL` for General.
    pub fn stored_id(&self) -> Option<Uuid> {
        (!self.is_general).then_some(self.id)
    }

    pub fn summary(&self) -> TopicSummary {
        TopicSummary {
            id: self.id,
            title: self.title.clone(),
            icon_emoji: self.icon_emoji.clone(),
            closed: self.closed_at.is_some(),
            pinned: self.pinned_at.is_some(),
            icon_color: Some(self.icon_color),
            icon_custom_emoji_id: self.icon_custom_emoji_id,
            hidden: self.is_hidden,
            is_general: self.is_general,
            deleted: false,
        }
    }
}

/// The new state of a topic after an edit, validated.
struct TopicChange {
    title: String,
    icon_emoji: String,
    icon_custom_emoji_id: Option<Uuid>,
    icon_color: i64,
    pinned_at: Option<DateTime<Utc>>,
    closed_at: Option<DateTime<Utc>>,
    is_hidden: bool,
}

impl TopicChange {
    fn from_request(row: &TopicRow, request: &UpdateTopicRequest) -> Result<Self, TopicError> {
        let now = Utc::now();
        if request.is_hidden.is_some() && !row.is_general {
            return Err(TopicError::Invalid("only General can be hidden"));
        }
        Ok(Self {
            title: match &request.title {
                Some(title) => valid_title(title)?,
                None => row.title.clone(),
            },
            icon_emoji: match &request.icon_emoji {
                Some(emoji) => valid_icon_emoji(emoji)?,
                None => row.icon_emoji.clone(),
            },
            icon_custom_emoji_id: request
                .icon_custom_emoji_id
                .unwrap_or(row.icon_custom_emoji_id),
            icon_color: match request.icon_color {
                Some(color) => valid_color(color)?,
                None => row.icon_color,
            },
            pinned_at: match request.is_pinned {
                Some(true) => row.pinned_at.or(Some(now)),
                Some(false) => None,
                None => row.pinned_at,
            },
            closed_at: match request.is_closed {
                Some(true) => row.closed_at.or(Some(now)),
                Some(false) => None,
                None => row.closed_at,
            },
            is_hidden: request.is_hidden.unwrap_or(row.is_hidden),
        })
    }
}

impl AppState {
    /// `Some(is_forum)` for a live chat, `None` when it does not exist.
    pub(crate) async fn chat_is_forum(&self, room_id: Uuid) -> Result<Option<bool>, sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query_scalar("SELECT is_forum FROM chats WHERE id = $1 AND deleted_at IS NULL")
                .bind(room_id)
                .fetch_optional(pool)
                .await
        })
    }

    pub(crate) async fn load_topic(
        &self,
        room_id: Uuid,
        topic_id: Uuid,
    ) -> Result<Option<TopicRow>, sqlx::Error> {
        let query = format!("{TOPIC_SELECT} WHERE id = $1 AND room_id = $2");
        with_pool!(self, |pool| {
            sqlx::query_as(&query)
                .bind(topic_id)
                .bind(room_id)
                .fetch_optional(pool)
                .await
        })
    }

    pub(crate) async fn chat_topics(&self, room_id: Uuid) -> Result<Vec<TopicRow>, sqlx::Error> {
        let query = format!("{TOPIC_SELECT} WHERE room_id = $1 ORDER BY created_at, id");
        with_pool!(self, |pool| {
            sqlx::query_as(&query).bind(room_id).fetch_all(pool).await
        })
    }

    /// The chat's General topic, created on first use. Idempotent under concurrency: the
    /// partial unique index admits one General per chat and the loser's insert is a no-op.
    pub(crate) async fn ensure_general_topic(
        &self,
        room_id: Uuid,
    ) -> Result<TopicRow, sqlx::Error> {
        let query = format!("{TOPIC_SELECT} WHERE room_id = $1 AND is_general");
        let existing: Option<TopicRow> = with_pool!(self, |pool| {
            sqlx::query_as(&query)
                .bind(room_id)
                .fetch_optional(pool)
                .await
        })?;
        if let Some(existing) = existing {
            return Ok(existing);
        }
        let id = Uuid::new_v4();
        with_pool!(self, |pool| {
            sqlx::query(
                "INSERT INTO forum_topics (id, room_id, is_general, title, icon_emoji, \
                 icon_color, is_hidden, created_at) VALUES ($1, $2, TRUE, $3, '', $4, FALSE, $5) \
                 ON CONFLICT (room_id) WHERE is_general DO NOTHING",
            )
            .bind(id)
            .bind(room_id)
            .bind(GENERAL_TOPIC_TITLE)
            .bind(default_topic_color(id))
            .bind(Utc::now())
            .execute(pool)
            .await
            .map(|_| ())
        })?;
        with_pool!(self, |pool| {
            sqlx::query_as(&query).bind(room_id).fetch_one(pool).await
        })
    }

    pub(crate) async fn insert_topic(
        &self,
        room_id: Uuid,
        creator_id: Uuid,
        title: String,
        icon_emoji: String,
        icon_custom_emoji_id: Option<Uuid>,
        icon_color: Option<i64>,
    ) -> Result<TopicRow, TopicError> {
        let id = Uuid::new_v4();
        let icon_color = match icon_color {
            Some(color) => valid_color(color)?,
            None => default_topic_color(id),
        };
        let row = TopicRow {
            id,
            room_id,
            is_general: false,
            title: valid_title(&title)?,
            icon_emoji: valid_icon_emoji(&icon_emoji)?,
            icon_custom_emoji_id,
            icon_color,
            pinned_at: None,
            closed_at: None,
            is_hidden: false,
            creator_id: Some(creator_id),
            created_at: Utc::now(),
        };
        with_pool!(self, |pool| {
            sqlx::query(
                "INSERT INTO forum_topics (id, room_id, is_general, title, icon_emoji, \
                 icon_custom_emoji_id, icon_color, is_hidden, creator_id, created_at) \
                 VALUES ($1, $2, FALSE, $3, $4, $5, $6, FALSE, $7, $8)",
            )
            .bind(row.id)
            .bind(row.room_id)
            .bind(&row.title)
            .bind(&row.icon_emoji)
            .bind(row.icon_custom_emoji_id)
            .bind(row.icon_color)
            .bind(row.creator_id)
            .bind(row.created_at)
            .execute(pool)
            .await
            .map(|_| ())
        })?;
        Ok(row)
    }

    pub(crate) async fn update_topic(
        &self,
        row: &TopicRow,
        request: &UpdateTopicRequest,
    ) -> Result<TopicRow, TopicError> {
        let change = TopicChange::from_request(row, request)?;
        with_pool!(self, |pool| {
            sqlx::query(
                "UPDATE forum_topics SET title = $1, icon_emoji = $2, icon_custom_emoji_id = $3, \
                 icon_color = $4, pinned_at = $5, closed_at = $6, is_hidden = $7 \
                 WHERE id = $8 AND room_id = $9",
            )
            .bind(&change.title)
            .bind(&change.icon_emoji)
            .bind(change.icon_custom_emoji_id)
            .bind(change.icon_color)
            .bind(change.pinned_at)
            .bind(change.closed_at)
            .bind(change.is_hidden)
            .bind(row.id)
            .bind(row.room_id)
            .execute(pool)
            .await
            .map(|_| ())
        })?;
        Ok(TopicRow {
            title: change.title,
            icon_emoji: change.icon_emoji,
            icon_custom_emoji_id: change.icon_custom_emoji_id,
            icon_color: change.icon_color,
            pinned_at: change.pinned_at,
            closed_at: change.closed_at,
            is_hidden: change.is_hidden,
            ..row.clone()
        })
    }

    /// Delete a topic and every message in it (Telegram's semantics), in one transaction.
    /// Returns the attachments those messages referenced, whose orphan state the caller
    /// recomputes afterwards: a forwarded copy or a favorite may still hold the same file.
    /// The dependent rows (reactions, pins, mentions, polls, entities, notifications, reply
    /// links, read cursors) follow through their existing `ON DELETE` rules.
    pub(crate) async fn delete_topic_with_messages(
        &self,
        row: &TopicRow,
    ) -> Result<Vec<Uuid>, sqlx::Error> {
        with_pool!(self, |pool| {
            async {
                let mut transaction = pool.begin().await?;
                let attachments: Vec<Option<Uuid>> = sqlx::query_scalar(
                    "DELETE FROM messages WHERE room_id = $1 AND topic_id = $2 \
                     RETURNING attachment_id",
                )
                .bind(row.room_id)
                .bind(row.id)
                .fetch_all(&mut *transaction)
                .await?;
                sqlx::query("DELETE FROM forum_topics WHERE id = $1 AND room_id = $2")
                    .bind(row.id)
                    .bind(row.room_id)
                    .execute(&mut *transaction)
                    .await?;
                transaction.commit().await?;
                let mut attachments: Vec<Uuid> = attachments.into_iter().flatten().collect();
                attachments.sort();
                attachments.dedup();
                Ok(attachments)
            }
            .await
        })
    }
}
