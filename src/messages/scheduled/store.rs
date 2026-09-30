//! `scheduled_messages` persistence. Every author-facing query filters by `sender_id`, so a
//! scheduled message is never readable or writable by anyone but its author.

use chrono::{DateTime, Utc};
use sqlx::FromRow;
use uuid::Uuid;

use super::model::ScheduledMessage;
use crate::state::{with_pool, AppState};
use crate::stickers::custom_emoji::{entity_store::insert_message_entities, MessageEntity};

const SCHEDULED_SELECT: &str = "SELECT id, room_id, sender_id, content, entities, reply_to_id, \
    silent, scheduled_at, created_at, updated_at, topic_id FROM scheduled_messages";

#[derive(Debug, Clone, FromRow)]
pub(crate) struct ScheduledRow {
    pub id: Uuid,
    pub room_id: Uuid,
    pub sender_id: Uuid,
    pub content: String,
    pub entities: String,
    pub reply_to_id: Option<Uuid>,
    pub silent: bool,
    pub scheduled_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// TG-204: the forum topic; `None` = General.
    pub topic_id: Option<Uuid>,
}

impl ScheduledRow {
    pub(crate) fn entities(&self) -> Vec<MessageEntity> {
        serde_json::from_str(&self.entities).unwrap_or_default()
    }

    pub(crate) fn into_view(self) -> ScheduledMessage {
        ScheduledMessage {
            entities: self.entities(),
            id: self.id,
            chat_id: self.room_id,
            content: self.content,
            reply_to: self.reply_to_id,
            silent: self.silent,
            topic_id: self.topic_id,
            scheduled_at: self.scheduled_at,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

/// The fields an update may change, already validated.
pub(crate) struct ScheduledPatch {
    pub content: String,
    pub entities: String,
    pub scheduled_at: DateTime<Utc>,
    pub silent: bool,
}

/// What delivery writes into `messages`.
pub(crate) struct Delivery<'a> {
    pub row: &'a ScheduledRow,
    pub sender_name: &'a str,
    pub reply_to: Option<Uuid>,
    pub entities: &'a [MessageEntity],
    pub created_at: DateTime<Utc>,
}

/// Outcome of one delivery transaction.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Delivered {
    /// The message was inserted under the scheduled id.
    Inserted,
    /// The scheduled row was consumed, but the author is no longer an active member.
    Dropped,
    /// Someone else already delivered, edited away or deleted the row.
    Gone,
}

impl AppState {
    pub(crate) async fn count_scheduled(
        &self,
        room_id: Uuid,
        sender_id: Uuid,
    ) -> Result<i64, sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query_scalar(
                "SELECT COUNT(*) FROM scheduled_messages WHERE room_id = $1 AND sender_id = $2",
            )
            .bind(room_id)
            .bind(sender_id)
            .fetch_one(pool)
            .await
        })
    }

    pub(crate) async fn insert_scheduled(&self, row: &ScheduledRow) -> Result<(), sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query(
                "INSERT INTO scheduled_messages (id, room_id, sender_id, content, entities, \
                 reply_to_id, silent, scheduled_at, created_at, updated_at, topic_id) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)",
            )
            .bind(row.id)
            .bind(row.room_id)
            .bind(row.sender_id)
            .bind(&row.content)
            .bind(&row.entities)
            .bind(row.reply_to_id)
            .bind(row.silent)
            .bind(row.scheduled_at)
            .bind(row.created_at)
            .bind(row.updated_at)
            .bind(row.topic_id)
            .execute(pool)
            .await
            .map(|_| ())
        })
    }

    pub(crate) async fn list_scheduled(
        &self,
        room_id: Uuid,
        sender_id: Uuid,
    ) -> Result<Vec<ScheduledRow>, sqlx::Error> {
        let query = format!(
            "{SCHEDULED_SELECT} WHERE room_id = $1 AND sender_id = $2 \
             ORDER BY scheduled_at, created_at, id"
        );
        with_pool!(self, |pool| {
            sqlx::query_as(&query)
                .bind(room_id)
                .bind(sender_id)
                .fetch_all(pool)
                .await
        })
    }

    /// The author's own scheduled message in `room_id`; `None` for anyone else.
    pub(crate) async fn own_scheduled(
        &self,
        room_id: Uuid,
        id: Uuid,
        sender_id: Uuid,
    ) -> Result<Option<ScheduledRow>, sqlx::Error> {
        let query = format!("{SCHEDULED_SELECT} WHERE id = $1 AND room_id = $2 AND sender_id = $3");
        with_pool!(self, |pool| {
            sqlx::query_as(&query)
                .bind(id)
                .bind(room_id)
                .bind(sender_id)
                .fetch_optional(pool)
                .await
        })
    }

    pub(crate) async fn scheduled_by_id(
        &self,
        id: Uuid,
    ) -> Result<Option<ScheduledRow>, sqlx::Error> {
        let query = format!("{SCHEDULED_SELECT} WHERE id = $1");
        with_pool!(self, |pool| {
            sqlx::query_as(&query).bind(id).fetch_optional(pool).await
        })
    }

    pub(crate) async fn update_scheduled(
        &self,
        id: Uuid,
        sender_id: Uuid,
        patch: &ScheduledPatch,
    ) -> Result<bool, sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query(
                "UPDATE scheduled_messages SET content = $1, entities = $2, scheduled_at = $3, \
                 silent = $4, updated_at = $5 WHERE id = $6 AND sender_id = $7",
            )
            .bind(&patch.content)
            .bind(&patch.entities)
            .bind(patch.scheduled_at)
            .bind(patch.silent)
            .bind(Utc::now())
            .bind(id)
            .bind(sender_id)
            .execute(pool)
            .await
            .map(|result| result.rows_affected() > 0)
        })
    }

    pub(crate) async fn delete_scheduled(
        &self,
        room_id: Uuid,
        id: Uuid,
        sender_id: Uuid,
    ) -> Result<bool, sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query(
                "DELETE FROM scheduled_messages WHERE id = $1 AND room_id = $2 AND sender_id = $3",
            )
            .bind(id)
            .bind(room_id)
            .bind(sender_id)
            .execute(pool)
            .await
            .map(|result| result.rows_affected() > 0)
        })
    }

    /// Scheduled messages whose time has come, oldest first.
    pub(crate) async fn due_scheduled_ids(
        &self,
        now: DateTime<Utc>,
        limit: i64,
    ) -> Result<Vec<Uuid>, sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query_scalar(
                "SELECT id FROM scheduled_messages WHERE scheduled_at <= $1 \
                 ORDER BY scheduled_at, id LIMIT $2",
            )
            .bind(now)
            .bind(limit)
            .fetch_all(pool)
            .await
        })
    }

    /// Consume the scheduled row and insert the message under the same id, atomically. The
    /// `DELETE` comes first: it takes the write lock (SQLite) or the row lock (PostgreSQL), so a
    /// concurrent deliverer waits and then finds nothing to consume — delivery is exactly once.
    pub(crate) async fn deliver_scheduled_row(
        &self,
        delivery: Delivery<'_>,
    ) -> Result<Delivered, sqlx::Error> {
        let row = delivery.row;
        with_pool!(self, |pool| {
            async {
                let mut tx = pool.begin().await?;
                let consumed = sqlx::query("DELETE FROM scheduled_messages WHERE id = $1")
                    .bind(row.id)
                    .execute(&mut *tx)
                    .await?
                    .rows_affected()
                    > 0;
                if !consumed {
                    return Ok::<_, sqlx::Error>(Delivered::Gone);
                }
                let inserted = sqlx::query(
                    "INSERT INTO messages (id, room_id, sender_id, sender, content, reply_to_id, \
                     silent, created_at, topic_id) \
                     SELECT $1, $2, $3, $4, $5, $6, $7, $8, $9 \
                     WHERE EXISTS (SELECT 1 FROM chat_members WHERE chat_members.room_id = $2 \
                       AND chat_members.user_id = $3 AND chat_members.status = 'active') \
                     AND EXISTS (SELECT 1 FROM chats WHERE chats.id = $2 \
                       AND chats.deleted_at IS NULL)",
                )
                .bind(row.id)
                .bind(row.room_id)
                .bind(row.sender_id)
                .bind(delivery.sender_name)
                .bind(&row.content)
                .bind(delivery.reply_to)
                .bind(row.silent)
                .bind(delivery.created_at)
                .bind(row.topic_id)
                .execute(&mut *tx)
                .await?
                .rows_affected()
                    > 0;
                if inserted {
                    insert_message_entities!(tx, row.id, delivery.entities)?;
                }
                tx.commit().await?;
                Ok(if inserted {
                    Delivered::Inserted
                } else {
                    Delivered::Dropped
                })
            }
            .await
        })
    }
}
