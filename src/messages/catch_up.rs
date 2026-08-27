use uuid::Uuid;

use super::store::{MessageRow, MESSAGE_SELECT};
use crate::{
    models::StoredMessage,
    state::{with_pool, AppState},
};

#[derive(Clone, Copy)]
pub(crate) struct CatchUpWindow {
    pub after_message_id: Option<Uuid>,
    pub through_message_id: Uuid,
    pub unread_message_count: i64,
}

impl AppState {
    pub(crate) async fn catch_up_window(
        &self,
        room_id: Uuid,
        user_id: Uuid,
    ) -> Result<Option<CatchUpWindow>, sqlx::Error> {
        let after_message_id = with_pool!(self, |pool| {
            sqlx::query_scalar(
                "SELECT message_id FROM room_reads WHERE room_id = $1 AND user_id = $2",
            )
            .bind(room_id)
            .bind(user_id)
            .fetch_optional(pool)
            .await
        })?;
        let through_message_id = with_pool!(self, |pool| {
            sqlx::query_scalar(
                "SELECT id FROM messages WHERE room_id = $1 AND recalled_at IS NULL \
                 ORDER BY created_at DESC, id DESC LIMIT 1",
            )
            .bind(room_id)
            .fetch_optional(pool)
            .await
        })?;
        let Some(through_message_id) = through_message_id else {
            return Ok(None);
        };
        let unread_message_count = self
            .count_catch_up_messages(room_id, user_id, after_message_id, through_message_id)
            .await?;
        Ok((unread_message_count > 0).then_some(CatchUpWindow {
            after_message_id,
            through_message_id,
            unread_message_count,
        }))
    }

    async fn count_catch_up_messages(
        &self,
        room_id: Uuid,
        user_id: Uuid,
        after_message_id: Option<Uuid>,
        through_message_id: Uuid,
    ) -> Result<i64, sqlx::Error> {
        with_pool!(self, |pool| {
            match after_message_id {
                Some(after) => sqlx::query_scalar(
                    "SELECT COUNT(*) FROM messages AS candidate, messages AS start_boundary, \
                     messages AS end_boundary WHERE candidate.room_id = $1 \
                     AND (candidate.sender_id IS NULL OR candidate.sender_id <> $2) \
                     AND candidate.recalled_at IS NULL AND start_boundary.id = $3 \
                     AND end_boundary.id = $4 \
                     AND (candidate.created_at > start_boundary.created_at OR \
                       (candidate.created_at = start_boundary.created_at AND candidate.id > start_boundary.id)) \
                     AND (candidate.created_at < end_boundary.created_at OR \
                       (candidate.created_at = end_boundary.created_at AND candidate.id <= end_boundary.id))",
                )
                .bind(room_id)
                .bind(user_id)
                .bind(after)
                .bind(through_message_id)
                .fetch_one(pool)
                .await,
                None => sqlx::query_scalar(
                    "SELECT COUNT(*) FROM messages AS candidate, messages AS end_boundary \
                     WHERE candidate.room_id = $1 \
                     AND (candidate.sender_id IS NULL OR candidate.sender_id <> $2) \
                     AND candidate.recalled_at IS NULL AND end_boundary.id = $3 \
                     AND (candidate.created_at < end_boundary.created_at OR \
                       (candidate.created_at = end_boundary.created_at AND candidate.id <= end_boundary.id))",
                )
                .bind(room_id)
                .bind(user_id)
                .bind(through_message_id)
                .fetch_one(pool)
                .await,
            }
        })
    }

    pub(crate) async fn catch_up_messages(
        &self,
        room_id: Uuid,
        user_id: Uuid,
        after_message_id: Option<Uuid>,
        through_message_id: Uuid,
        limit: usize,
    ) -> Result<Vec<StoredMessage>, sqlx::Error> {
        let limit = i64::try_from(limit.min(500)).unwrap_or(500).max(1);
        let membership = "EXISTS (SELECT 1 FROM room_memberships membership \
            WHERE membership.room_id = $1 AND membership.user_id = $2 \
            AND membership.status = 'active')";
        let query = match after_message_id {
            Some(_) => format!(
                "SELECT * FROM ({MESSAGE_SELECT} WHERE messages.room_id = $1 \
                 AND messages.recalled_at IS NULL \
                 AND (messages.sender_id IS NULL OR messages.sender_id <> $2) AND {membership} \
                 AND (messages.created_at > (SELECT created_at FROM messages WHERE id = $4 AND room_id = $1) OR \
                   (messages.created_at = (SELECT created_at FROM messages WHERE id = $4 AND room_id = $1) \
                    AND messages.id > $4)) \
                 AND (messages.created_at < (SELECT created_at FROM messages WHERE id = $3 AND room_id = $1) OR \
                   (messages.created_at = (SELECT created_at FROM messages WHERE id = $3 AND room_id = $1) \
                    AND messages.id <= $3)) \
                 ORDER BY messages.created_at DESC, messages.id DESC LIMIT $5) bounded \
                 ORDER BY created_at ASC, id ASC"
            ),
            None => format!(
                "SELECT * FROM ({MESSAGE_SELECT} WHERE messages.room_id = $1 \
                 AND messages.recalled_at IS NULL \
                 AND (messages.sender_id IS NULL OR messages.sender_id <> $2) AND {membership} \
                 AND (messages.created_at < (SELECT created_at FROM messages WHERE id = $3 AND room_id = $1) OR \
                   (messages.created_at = (SELECT created_at FROM messages WHERE id = $3 AND room_id = $1) \
                    AND messages.id <= $3)) \
                 ORDER BY messages.created_at DESC, messages.id DESC LIMIT $4) bounded \
                 ORDER BY created_at ASC, id ASC"
            ),
        };
        let rows: Vec<MessageRow> = with_pool!(self, |pool| {
            match after_message_id {
                Some(after) => {
                    sqlx::query_as(&query)
                        .bind(room_id)
                        .bind(user_id)
                        .bind(through_message_id)
                        .bind(after)
                        .bind(limit)
                        .fetch_all(pool)
                        .await
                }
                None => {
                    sqlx::query_as(&query)
                        .bind(room_id)
                        .bind(user_id)
                        .bind(through_message_id)
                        .bind(limit)
                        .fetch_all(pool)
                        .await
                }
            }
        })?;
        let mut messages = rows
            .into_iter()
            .map(|row| row.into_message(Some(user_id)))
            .collect::<Vec<_>>();
        self.attach_message_reactions(&mut messages).await?;
        Ok(messages)
    }
}
