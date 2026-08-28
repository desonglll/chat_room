use std::collections::HashMap;

use chrono::{Duration, Utc};
use sqlx::{Postgres, QueryBuilder, Sqlite};
use uuid::Uuid;

use super::RetrievedMessage;
use crate::state::{with_pool, AppState};

#[derive(sqlx::FromRow)]
pub(super) struct IndexJob {
    pub message_id: Uuid,
    pub operation: String,
    pub attempt_count: i64,
    pub generation: i64,
}

#[derive(sqlx::FromRow)]
struct StoredIndexJob {
    message_id: String,
    operation: String,
    attempt_count: i64,
    generation: i64,
}

impl TryFrom<StoredIndexJob> for IndexJob {
    type Error = sqlx::Error;

    fn try_from(stored: StoredIndexJob) -> Result<Self, Self::Error> {
        Ok(Self {
            message_id: Uuid::parse_str(&stored.message_id)
                .map_err(|error| sqlx::Error::Decode(Box::new(error)))?,
            operation: stored.operation,
            attempt_count: stored.attempt_count,
            generation: stored.generation,
        })
    }
}

#[derive(sqlx::FromRow)]
pub(super) struct IndexedMessage {
    pub id: Uuid,
    pub room_id: Uuid,
    pub content: String,
}

impl AppState {
    pub(super) async fn ready_index_jobs(&self) -> Result<Vec<IndexJob>, sqlx::Error> {
        match self.database_pool() {
            crate::storage::DatabasePool::Sqlite(pool) => {
                let stored: Vec<StoredIndexJob> = sqlx::query_as(
                    "SELECT CASE WHEN typeof(message_id) = 'blob' THEN lower(hex(message_id)) \
                            ELSE message_id END AS message_id, \
                            operation, attempt_count, generation \
                     FROM message_index_outbox WHERE next_attempt_at <= $1 \
                     ORDER BY updated_at ASC LIMIT 20",
                )
                .bind(Utc::now())
                .fetch_all(pool)
                .await?;
                stored.into_iter().map(IndexJob::try_from).collect()
            }
            crate::storage::DatabasePool::Postgres(pool) => {
                sqlx::query_as(
                    "SELECT message_id, operation, attempt_count, generation \
                     FROM message_index_outbox WHERE next_attempt_at <= $1 \
                     ORDER BY updated_at ASC LIMIT 20",
                )
                .bind(Utc::now())
                .fetch_all(pool)
                .await
            }
        }
    }

    pub(super) async fn indexed_message(
        &self,
        message_id: Uuid,
    ) -> Result<Option<IndexedMessage>, sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query_as(
                "SELECT id, room_id, \
                   CASE WHEN trim(content) <> '' AND trim(visual_text) <> '' \
                     THEN content || '\n\nVisual projection:\n' || visual_text \
                     WHEN trim(visual_text) <> '' THEN visual_text ELSE content END AS content \
                 FROM (SELECT messages.id, messages.room_id, messages.content, \
                   CASE WHEN attachments.is_sensitive = FALSE THEN COALESCE(( \
                     SELECT projections.search_text FROM attachment_visual_projections projections \
                     WHERE projections.attachment_id = attachments.id \
                     ORDER BY projections.updated_at DESC LIMIT 1), '') ELSE '' END AS visual_text \
                   FROM messages LEFT JOIN attachments ON attachments.id = messages.attachment_id \
                   WHERE messages.id = $1 AND messages.recalled_at IS NULL) indexed_message_row \
                 WHERE trim(content) <> '' OR trim(visual_text) <> ''",
            )
            .bind(message_id)
            .fetch_optional(pool)
            .await
        })
    }

    pub(super) async fn complete_index_job(&self, job: &IndexJob) -> Result<(), sqlx::Error> {
        match self.database_pool() {
            crate::storage::DatabasePool::Sqlite(pool) => sqlx::query(
                "DELETE FROM message_index_outbox \
                 WHERE (message_id = $1 OR message_id = $2) AND generation = $3",
            )
            .bind(job.message_id)
            .bind(job.message_id.to_string())
            .bind(job.generation)
            .execute(pool)
            .await
            .map(|_| ()),
            crate::storage::DatabasePool::Postgres(pool) => sqlx::query(
                "DELETE FROM message_index_outbox WHERE message_id = $1 AND generation = $2",
            )
            .bind(job.message_id)
            .bind(job.generation)
            .execute(pool)
            .await
            .map(|_| ()),
        }
    }

    pub(super) async fn retry_index_job(
        &self,
        job: &IndexJob,
        error: &str,
    ) -> Result<(), sqlx::Error> {
        let exponent = u32::try_from(job.attempt_count.clamp(0, 8)).unwrap_or(8);
        let next_attempt_at = Utc::now() + Duration::seconds(2_i64.pow(exponent).clamp(1, 300));
        let last_error: String = error.chars().take(300).collect();
        match self.database_pool() {
            crate::storage::DatabasePool::Sqlite(pool) => sqlx::query(
                "UPDATE message_index_outbox SET attempt_count = attempt_count + 1, \
                 next_attempt_at = $1, last_error = $2, updated_at = $3 \
                 WHERE (message_id = $4 OR message_id = $5) AND generation = $6",
            )
            .bind(next_attempt_at)
            .bind(&last_error)
            .bind(Utc::now())
            .bind(job.message_id)
            .bind(job.message_id.to_string())
            .bind(job.generation)
            .execute(pool)
            .await
            .map(|_| ()),
            crate::storage::DatabasePool::Postgres(pool) => sqlx::query(
                "UPDATE message_index_outbox SET attempt_count = attempt_count + 1, \
                 next_attempt_at = $1, last_error = $2, updated_at = $3 \
                 WHERE message_id = $4 AND generation = $5",
            )
            .bind(next_attempt_at)
            .bind(last_error)
            .bind(Utc::now())
            .bind(job.message_id)
            .bind(job.generation)
            .execute(pool)
            .await
            .map(|_| ()),
        }
    }

    pub(crate) async fn authorized_retrieved_messages(
        &self,
        user_id: Uuid,
        room_id: Uuid,
        message_ids: &[Uuid],
    ) -> Result<Vec<RetrievedMessage>, sqlx::Error> {
        if message_ids.is_empty() {
            return Ok(Vec::new());
        }
        let rows = match self.database_pool() {
            crate::storage::DatabasePool::Sqlite(pool) => {
                authorized_messages_sqlite(pool, user_id, room_id, message_ids).await?
            }
            crate::storage::DatabasePool::Postgres(pool) => {
                authorized_messages_postgres(pool, user_id, room_id, message_ids).await?
            }
        };
        let mut by_id: HashMap<Uuid, RetrievedMessage> = rows
            .into_iter()
            .map(|message| (message.id, message))
            .collect();
        Ok(message_ids
            .iter()
            .filter_map(|message_id| by_id.remove(message_id))
            .collect())
    }
}

async fn authorized_messages_sqlite(
    pool: &sqlx::SqlitePool,
    user_id: Uuid,
    room_id: Uuid,
    message_ids: &[Uuid],
) -> Result<Vec<RetrievedMessage>, sqlx::Error> {
    let mut query = authorized_messages_query::<Sqlite>(user_id, room_id, message_ids);
    query.build_query_as().fetch_all(pool).await
}

async fn authorized_messages_postgres(
    pool: &sqlx::PgPool,
    user_id: Uuid,
    room_id: Uuid,
    message_ids: &[Uuid],
) -> Result<Vec<RetrievedMessage>, sqlx::Error> {
    let mut query = authorized_messages_query::<Postgres>(user_id, room_id, message_ids);
    query.build_query_as().fetch_all(pool).await
}

fn authorized_messages_query<DB>(
    user_id: Uuid,
    room_id: Uuid,
    message_ids: &[Uuid],
) -> QueryBuilder<'static, DB>
where
    DB: sqlx::Database,
    Uuid: for<'query> sqlx::Encode<'query, DB> + sqlx::Type<DB>,
{
    let mut query = QueryBuilder::<DB>::new(
        "SELECT messages.id, messages.sender, messages.content, messages.created_at, \
         attachments.id AS attachment_id, attachments.access_key AS attachment_access_key, \
         attachments.file_name AS attachment_file_name, attachments.mime_type AS attachment_mime_type, \
         attachments.size_bytes AS attachment_size_bytes, \
         attachments.is_sensitive AS attachment_is_sensitive, \
         CASE WHEN attachments.is_sensitive = FALSE THEN (SELECT projections.search_text \
           FROM attachment_visual_projections projections \
           WHERE projections.attachment_id = attachments.id \
           ORDER BY projections.updated_at DESC LIMIT 1) ELSE NULL END AS attachment_visual_text \
         FROM messages LEFT JOIN attachments ON attachments.id = messages.attachment_id \
         WHERE messages.room_id = ",
    );
    query
        .push_bind(room_id)
        .push(
            " AND messages.recalled_at IS NULL AND EXISTS (SELECT 1 FROM room_memberships \
               WHERE room_memberships.room_id = messages.room_id \
                 AND room_memberships.user_id = ",
        )
        .push_bind(user_id)
        .push(" AND room_memberships.status = 'active') AND messages.id IN (");
    {
        let mut values = query.separated(", ");
        for message_id in message_ids {
            values.push_bind(*message_id);
        }
    }
    query.push(")");
    query
}

#[cfg(test)]
#[path = "store_tests.rs"]
mod tests;
