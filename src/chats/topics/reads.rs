//! Per-topic read cursors, unread counts and mute — one account's state in one topic,
//! kept apart from the chat-level `chat_reads` cursor and `chat_members.muted_until`.
//!
//! A topic's *effective* cursor is the later of its own cursor and the chat-level cursor, so
//! a chat that turns into a forum does not suddenly show its whole history as unread, and a
//! frozen client that reads the chat as one stream still reads every topic. The forum client
//! reads topic by topic and never sends a chat-level read; the chat cursor follows only when
//! a topic read leaves nothing unread anywhere ([`AppState::mark_topic_read`]).

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use uuid::Uuid;

use super::model::{TopicError, TopicReadResult};
use super::store::TopicRow;
use crate::state::{with_pool, AppState};

/// `muted`, `muted_until` of one account in one topic.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct TopicMute {
    pub muted: bool,
    pub muted_until: Option<DateTime<Utc>>,
}

impl TopicMute {
    pub fn active(&self, now: DateTime<Utc>) -> bool {
        self.muted && self.muted_until.is_none_or(|until| until > now)
    }
}

type MuteRow = (Uuid, bool, Option<DateTime<Utc>>);

impl AppState {
    /// Unread messages per stored topic id (`None` = General) for `viewer_id`: other people's
    /// unrecalled messages after the topic's effective cursor.
    pub(crate) async fn topic_unread_counts(
        &self,
        room_id: Uuid,
        viewer_id: Uuid,
        general_id: Uuid,
    ) -> Result<HashMap<Option<Uuid>, i64>, sqlx::Error> {
        let rows: Vec<(Option<Uuid>, i64)> = with_pool!(self, |pool| {
            sqlx::query_as(
                "SELECT messages.topic_id, COUNT(messages.id) FROM messages \
                 LEFT JOIN forum_topic_members AS topic_read \
                   ON topic_read.user_id = $2 \
                   AND topic_read.topic_id = COALESCE(messages.topic_id, $3) \
                 LEFT JOIN chat_reads ON chat_reads.room_id = messages.room_id \
                   AND chat_reads.user_id = $2 \
                 LEFT JOIN messages AS chat_read ON chat_read.id = chat_reads.message_id \
                 WHERE messages.room_id = $1 AND messages.recalled_at IS NULL \
                   AND (messages.sender_id IS NULL OR messages.sender_id <> $2) \
                   AND (topic_read.read_created_at IS NULL \
                     OR messages.created_at > topic_read.read_created_at \
                     OR (messages.created_at = topic_read.read_created_at \
                       AND messages.id > topic_read.read_message_id)) \
                   AND (chat_read.id IS NULL OR messages.created_at > chat_read.created_at \
                     OR (messages.created_at = chat_read.created_at \
                       AND messages.id > chat_read.id)) \
                 GROUP BY messages.topic_id",
            )
            .bind(room_id)
            .bind(viewer_id)
            .bind(general_id)
            .fetch_all(pool)
            .await
        })?;
        Ok(rows.into_iter().collect())
    }

    pub(crate) async fn topic_mutes(
        &self,
        room_id: Uuid,
        user_id: Uuid,
    ) -> Result<HashMap<Uuid, TopicMute>, sqlx::Error> {
        let rows: Vec<MuteRow> = with_pool!(self, |pool| {
            sqlx::query_as(
                "SELECT forum_topic_members.topic_id, forum_topic_members.muted, \
                 forum_topic_members.muted_until FROM forum_topic_members \
                 JOIN forum_topics ON forum_topics.id = forum_topic_members.topic_id \
                 WHERE forum_topic_members.user_id = $1 AND forum_topics.room_id = $2",
            )
            .bind(user_id)
            .bind(room_id)
            .fetch_all(pool)
            .await
        })?;
        Ok(rows
            .into_iter()
            .map(|(topic_id, muted, muted_until)| (topic_id, TopicMute { muted, muted_until }))
            .collect())
    }

    /// Advance `user_id`'s cursor in `topic` to `message_id` (never backwards). The message
    /// must be in that topic. When the read leaves no unread message in any topic, the
    /// chat-level cursor follows to the chat's newest message.
    pub(crate) async fn mark_topic_read(
        &self,
        topic: &TopicRow,
        user_id: Uuid,
        message_id: Uuid,
    ) -> Result<TopicReadResult, TopicError> {
        let target: Option<(DateTime<Utc>, Option<Uuid>)> = with_pool!(self, |pool| {
            sqlx::query_as(
                "SELECT created_at, topic_id FROM messages WHERE id = $1 AND room_id = $2",
            )
            .bind(message_id)
            .bind(topic.room_id)
            .fetch_optional(pool)
            .await
        })?;
        let Some((created_at, stored_topic)) = target else {
            return Err(TopicError::NotFound);
        };
        if stored_topic != topic.stored_id() {
            return Err(TopicError::Invalid("the message is not in this topic"));
        }
        let now = Utc::now();
        with_pool!(self, |pool| {
            sqlx::query(
                "INSERT INTO forum_topic_members \
                 (topic_id, user_id, read_created_at, read_message_id, muted, updated_at) \
                 VALUES ($1, $2, $3, $4, FALSE, $5) \
                 ON CONFLICT (topic_id, user_id) DO UPDATE SET \
                   read_created_at = excluded.read_created_at, \
                   read_message_id = excluded.read_message_id, updated_at = excluded.updated_at \
                 WHERE forum_topic_members.read_created_at IS NULL \
                   OR excluded.read_created_at > forum_topic_members.read_created_at \
                   OR (excluded.read_created_at = forum_topic_members.read_created_at \
                     AND excluded.read_message_id > forum_topic_members.read_message_id)",
            )
            .bind(topic.id)
            .bind(user_id)
            .bind(created_at)
            .bind(message_id)
            .bind(now)
            .execute(pool)
            .await
            .map(|_| ())
        })?;
        let general = self.ensure_general_topic(topic.room_id).await?;
        let unread = self
            .topic_unread_counts(topic.room_id, user_id, general.id)
            .await?;
        let unread_count = unread.get(&topic.stored_id()).copied().unwrap_or(0);
        let mut chat_read_advanced = false;
        if unread.values().all(|count| *count == 0) {
            if let Some(newest) = self.latest_message_cursor(topic.room_id).await? {
                chat_read_advanced = self
                    .store_read_cursor(topic.room_id, user_id, newest.id)
                    .await?;
            }
        }
        Ok(TopicReadResult {
            topic_id: topic.id,
            unread_count,
            chat_read_advanced,
        })
    }

    pub(crate) async fn set_topic_mute(
        &self,
        topic_id: Uuid,
        user_id: Uuid,
        mute: TopicMute,
    ) -> Result<(), sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query(
                "INSERT INTO forum_topic_members (topic_id, user_id, muted, muted_until, updated_at) \
                 VALUES ($1, $2, $3, $4, $5) ON CONFLICT (topic_id, user_id) DO UPDATE SET \
                 muted = excluded.muted, muted_until = excluded.muted_until, \
                 updated_at = excluded.updated_at",
            )
            .bind(topic_id)
            .bind(user_id)
            .bind(mute.muted)
            .bind(mute.muted_until)
            .bind(Utc::now())
            .execute(pool)
            .await
            .map(|_| ())
        })?;
        Ok(())
    }

    /// Whether `user_id` muted the topic `message_id` was posted in (General included). Web
    /// push consults this after the chat-level preference.
    pub async fn message_topic_muted(
        &self,
        user_id: Uuid,
        message_id: Uuid,
    ) -> Result<bool, sqlx::Error> {
        let row: Option<(bool, Option<DateTime<Utc>>)> = with_pool!(self, |pool| {
            sqlx::query_as(
                "SELECT state.muted, state.muted_until FROM messages \
                 JOIN chats ON chats.id = messages.room_id AND chats.is_forum \
                 JOIN forum_topics ON forum_topics.room_id = messages.room_id \
                   AND ((messages.topic_id IS NULL AND forum_topics.is_general) \
                     OR forum_topics.id = messages.topic_id) \
                 JOIN forum_topic_members AS state ON state.topic_id = forum_topics.id \
                   AND state.user_id = $2 \
                 WHERE messages.id = $1",
            )
            .bind(message_id)
            .bind(user_id)
            .fetch_optional(pool)
            .await
        })?;
        Ok(row.is_some_and(|(muted, muted_until)| {
            TopicMute { muted, muted_until }.active(Utc::now())
        }))
    }
}
