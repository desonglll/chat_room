//! Topic-scoped message pages: the same contract as `GET /api/chats/:id/messages` and its
//! `/context` window, restricted to one topic (`topic_id IS NULL` for General).
//!
//! Deliberately a separate query rather than a filter on the chat-wide history, so that
//! handler and its Redis page cache stay untouched; topic pages are not cached.

use uuid::Uuid;

use super::store::TopicRow;
use crate::message_store::{MessageCursor, MessageRow, MESSAGE_SELECT};
use crate::models::StoredMessage;
use crate::state::{with_pool, AppState};

/// The SQL predicate selecting one topic's messages, with `$1` = room and `$2` = topic.
fn topic_filter(topic: &TopicRow) -> &'static str {
    if topic.is_general {
        "messages.room_id = $1 AND messages.topic_id IS NULL AND CAST($2 AS TEXT) IS NULL"
    } else {
        "messages.room_id = $1 AND messages.topic_id = $2"
    }
}

impl AppState {
    /// Where a message sits, provided it is in `topic`.
    pub(crate) async fn topic_message_cursor(
        &self,
        topic: &TopicRow,
        message_id: Uuid,
    ) -> Result<Option<MessageCursor>, sqlx::Error> {
        let row: Option<(chrono::DateTime<chrono::Utc>, Option<Uuid>)> =
            with_pool!(self, |pool| {
                sqlx::query_as(
                    "SELECT created_at, topic_id FROM messages WHERE id = $1 AND room_id = $2",
                )
                .bind(message_id)
                .bind(topic.room_id)
                .fetch_optional(pool)
                .await
            })?;
        Ok(row
            .filter(|(_, stored)| *stored == topic.stored_id())
            .map(|(created_at, _)| MessageCursor {
                created_at,
                id: message_id,
            }))
    }

    /// The newest `limit` messages of the topic at or before `through`, oldest first — the
    /// exact ordering and inclusive cursor of the chat-wide `message_history`.
    pub(crate) async fn topic_message_history(
        &self,
        topic: &TopicRow,
        limit: i64,
        through: Option<&MessageCursor>,
        viewer_id: Uuid,
    ) -> Result<Vec<StoredMessage>, sqlx::Error> {
        let limit = limit.clamp(1, 500);
        let filter = topic_filter(topic);
        let topic_bind = topic.stored_id();
        let mut rows: Vec<MessageRow> = match through {
            Some(cursor) => {
                let query = format!(
                    "{MESSAGE_SELECT} WHERE {filter} AND \
                     (messages.created_at < $3 OR (messages.created_at = $4 AND messages.id <= $5)) \
                     ORDER BY messages.created_at DESC, messages.id DESC LIMIT $6"
                );
                with_pool!(self, |pool| {
                    sqlx::query_as(&query)
                        .bind(topic.room_id)
                        .bind(topic_bind)
                        .bind(cursor.created_at)
                        .bind(cursor.created_at)
                        .bind(cursor.id)
                        .bind(limit)
                        .fetch_all(pool)
                        .await
                })?
            }
            None => {
                let query = format!(
                    "{MESSAGE_SELECT} WHERE {filter} \
                     ORDER BY messages.created_at DESC, messages.id DESC LIMIT $3"
                );
                with_pool!(self, |pool| {
                    sqlx::query_as(&query)
                        .bind(topic.room_id)
                        .bind(topic_bind)
                        .bind(limit)
                        .fetch_all(pool)
                        .await
                })?
            }
        };
        rows.reverse();
        self.finish_topic_page(rows, viewer_id).await
    }

    /// A window around `message_id` inside the topic (`None` when it is not in the topic).
    pub(crate) async fn topic_message_context(
        &self,
        topic: &TopicRow,
        message_id: Uuid,
        limit: i64,
        viewer_id: Uuid,
    ) -> Result<Option<Vec<StoredMessage>>, sqlx::Error> {
        let Some(cursor) = self.topic_message_cursor(topic, message_id).await? else {
            return Ok(None);
        };
        let limit = limit.clamp(1, 100);
        let mut rows: Vec<MessageRow> = Vec::new();
        let older = self
            .topic_message_history(topic, limit / 2 + 1, Some(&cursor), viewer_id)
            .await?;
        let newer_limit = limit.saturating_sub(older.len() as i64);
        let filter = topic_filter(topic);
        let newer_query = format!(
            "{MESSAGE_SELECT} WHERE {filter} AND \
             (messages.created_at > $3 OR (messages.created_at = $4 AND messages.id > $5)) \
             ORDER BY messages.created_at ASC, messages.id ASC LIMIT $6"
        );
        if newer_limit > 0 {
            rows = with_pool!(self, |pool| {
                sqlx::query_as(&newer_query)
                    .bind(topic.room_id)
                    .bind(topic.stored_id())
                    .bind(cursor.created_at)
                    .bind(cursor.created_at)
                    .bind(cursor.id)
                    .bind(newer_limit)
                    .fetch_all(pool)
                    .await
            })?;
        }
        let newer = self.finish_topic_page(rows, viewer_id).await?;
        Ok(Some(older.into_iter().chain(newer).collect()))
    }

    async fn finish_topic_page(
        &self,
        rows: Vec<MessageRow>,
        viewer_id: Uuid,
    ) -> Result<Vec<StoredMessage>, sqlx::Error> {
        let mut messages: Vec<StoredMessage> = rows
            .into_iter()
            .map(|row| row.into_message(Some(viewer_id)))
            .collect();
        self.attach_message_reactions(&mut messages).await?;
        self.attach_message_polls(&mut messages, Some(viewer_id))
            .await?;
        Ok(messages)
    }
}
