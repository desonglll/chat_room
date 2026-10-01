//! TG-203 channel comments: a channel's linked discussion group and the per-post threads in it.
//!
//! The copy of each post into the group, and its recall/edit/delete, are database triggers
//! (`migrations*/20270201000010_add_channel_discussions.sql`) so that every write path obeys
//! them. This module owns linking, reading a post's thread, and the comment count projection.

use std::collections::HashMap;

use sqlx::QueryBuilder;
use uuid::Uuid;

use super::ChatType;
use crate::messages::store::{MessageRow, MESSAGE_SELECT};
use crate::models::StoredMessage;
use crate::state::{with_pool, AppState};

/// The most comments one thread read returns (oldest first).
pub const THREAD_LIMIT: i64 = 500;

/// Why a link request was refused.
#[derive(Debug, PartialEq, Eq)]
pub enum LinkError {
    /// No such group, or it is not a group/supergroup.
    NotAGroup,
    /// The group is already some other channel's discussion group.
    AlreadyLinked,
}

/// A post's thread: where it lives in the discussion group.
#[derive(Debug, Clone, Copy)]
pub struct PostThread {
    pub discussion_chat_id: Uuid,
    pub discussion_message_id: Uuid,
}

const THREAD_CTE: &str = "WITH RECURSIVE thread(id) AS ( \
       SELECT messages.id FROM messages \
       WHERE messages.reply_to_id = $1 AND messages.room_id = $2 \
     UNION ALL \
       SELECT messages.id FROM messages JOIN thread ON messages.reply_to_id = thread.id \
       WHERE messages.room_id = $2 ) ";

impl AppState {
    /// Links `channel_id` to `group_id` (both ways), or unlinks it when `group_id` is `None`.
    /// The caller has authorized both sides. A channel has at most one group and a group serves
    /// at most one channel; relinking a channel releases its previous group.
    pub async fn set_discussion_group(
        &self,
        channel_id: Uuid,
        group_id: Option<Uuid>,
    ) -> Result<Result<(), LinkError>, sqlx::Error> {
        let previous = self
            .chat(channel_id)
            .await
            .and_then(|chat| chat.linked_chat_id);
        if let Some(group_id) = group_id {
            let Some(group) = self.chat(group_id).await else {
                return Ok(Err(LinkError::NotAGroup));
            };
            if !matches!(group.chat_type, ChatType::Group | ChatType::Supergroup) {
                return Ok(Err(LinkError::NotAGroup));
            }
            if group
                .linked_chat_id
                .is_some_and(|linked| linked != channel_id)
            {
                return Ok(Err(LinkError::AlreadyLinked));
            }
        }
        with_pool!(self, |pool| {
            async {
                let mut tx = pool.begin().await?;
                sqlx::query(
                    "UPDATE chats SET linked_chat_id = NULL \
                     WHERE linked_chat_id = $1 AND chat_type <> 'channel'",
                )
                .bind(channel_id)
                .execute(&mut *tx)
                .await?;
                sqlx::query("UPDATE chats SET linked_chat_id = $1 WHERE id = $2")
                    .bind(group_id)
                    .bind(channel_id)
                    .execute(&mut *tx)
                    .await?;
                if let Some(group_id) = group_id {
                    sqlx::query("UPDATE chats SET linked_chat_id = $1 WHERE id = $2")
                        .bind(channel_id)
                        .bind(group_id)
                        .execute(&mut *tx)
                        .await?;
                }
                tx.commit().await
            }
            .await
        })?;
        for (id, linked) in [
            (Some(channel_id), group_id),
            (previous.filter(|old| Some(*old) != group_id), None),
            (group_id, Some(channel_id)),
        ] {
            if let Some(mut chat) = match id {
                Some(id) => self.chat(id).await,
                None => None,
            } {
                chat.linked_chat_id = linked;
                self.cache_updated_chat(chat).await;
            }
        }
        Ok(Ok(()))
    }

    /// Where `post_id`'s comments live, if it is a live post of `channel_id` with a thread.
    pub async fn post_thread(
        &self,
        channel_id: Uuid,
        post_id: Uuid,
    ) -> Result<Option<PostThread>, sqlx::Error> {
        let row: Option<(Uuid, Uuid)> = with_pool!(self, |pool| {
            sqlx::query_as(
                "SELECT threads.discussion_chat_id, threads.discussion_message_id \
                 FROM channel_discussion_threads AS threads \
                 JOIN messages AS post ON post.id = threads.channel_message_id \
                 WHERE threads.channel_message_id = $1 AND threads.channel_id = $2 \
                   AND post.recalled_at IS NULL",
            )
            .bind(post_id)
            .bind(channel_id)
            .fetch_optional(pool)
            .await
        })?;
        Ok(
            row.map(|(discussion_chat_id, discussion_message_id)| PostThread {
                discussion_chat_id,
                discussion_message_id,
            }),
        )
    }

    /// The thread under `thread`'s root — replies and replies to replies — oldest first.
    pub async fn thread_messages(
        &self,
        thread: PostThread,
        viewer_id: Uuid,
    ) -> Result<Vec<StoredMessage>, sqlx::Error> {
        let query = format!(
            "{THREAD_CTE}{MESSAGE_SELECT} WHERE messages.id IN (SELECT id FROM thread) \
             ORDER BY messages.created_at, messages.id LIMIT {THREAD_LIMIT}"
        );
        let rows: Vec<MessageRow> = with_pool!(self, |pool| {
            sqlx::query_as(&query)
                .bind(thread.discussion_message_id)
                .bind(thread.discussion_chat_id)
                .fetch_all(pool)
                .await
        })?;
        let mut messages: Vec<StoredMessage> = rows
            .into_iter()
            .map(|row| row.into_message(Some(viewer_id)))
            .collect();
        self.attach_message_reactions(&mut messages).await?;
        self.attach_message_polls(&mut messages, Some(viewer_id))
            .await?;
        Ok(messages)
    }

    /// Whether `message_id` belongs to `thread` (the root itself or any reply in it).
    pub async fn in_thread(
        &self,
        thread: PostThread,
        message_id: Uuid,
    ) -> Result<bool, sqlx::Error> {
        if message_id == thread.discussion_message_id {
            return Ok(true);
        }
        let query = format!("{THREAD_CTE}SELECT 1 FROM thread WHERE id = $3 LIMIT 1");
        let found: Option<i32> = with_pool!(self, |pool| {
            sqlx::query_scalar(&query)
                .bind(thread.discussion_message_id)
                .bind(thread.discussion_chat_id)
                .bind(message_id)
                .fetch_optional(pool)
                .await
        })?;
        Ok(found.is_some())
    }

    /// Live comment counts for the posts among `post_ids` that have a thread (others absent).
    pub(crate) async fn comment_counts(
        &self,
        post_ids: &[Uuid],
    ) -> Result<HashMap<Uuid, i64>, sqlx::Error> {
        if post_ids.is_empty() {
            return Ok(HashMap::new());
        }
        let rows: Vec<(Uuid, i64)> = with_pool!(self, |pool| {
            let mut query = QueryBuilder::new(
                "WITH RECURSIVE thread(post, room, id) AS ( \
                   SELECT threads.channel_message_id, threads.discussion_chat_id, messages.id \
                   FROM channel_discussion_threads AS threads \
                   JOIN messages ON messages.reply_to_id = threads.discussion_message_id \
                     AND messages.room_id = threads.discussion_chat_id \
                   WHERE threads.channel_message_id IN (",
            );
            {
                let mut values = query.separated(", ");
                for id in post_ids {
                    values.push_bind(*id);
                }
            }
            query.push(
                ") UNION ALL \
                   SELECT thread.post, thread.room, messages.id FROM messages \
                   JOIN thread ON messages.reply_to_id = thread.id AND messages.room_id = thread.room ) \
                 SELECT threads.channel_message_id, \
                   CAST(COUNT(messages.id) AS BIGINT) \
                 FROM channel_discussion_threads AS threads \
                 LEFT JOIN thread ON thread.post = threads.channel_message_id \
                 LEFT JOIN messages ON messages.id = thread.id AND messages.recalled_at IS NULL \
                 WHERE threads.channel_message_id IN (",
            );
            {
                let mut values = query.separated(", ");
                for id in post_ids {
                    values.push_bind(*id);
                }
            }
            query.push(") GROUP BY threads.channel_message_id");
            query.build_query_as().fetch_all(pool).await
        })?;
        Ok(rows.into_iter().collect())
    }
}
