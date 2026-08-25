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
pub(super) struct IndexedMessage {
    pub id: Uuid,
    pub room_id: Uuid,
    pub content: String,
}

impl AppState {
    pub(super) async fn ready_index_jobs(&self) -> Result<Vec<IndexJob>, sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query_as(
                "SELECT message_id, operation, attempt_count, generation \
                 FROM message_index_outbox WHERE next_attempt_at <= $1 \
                 ORDER BY updated_at ASC LIMIT 20",
            )
            .bind(Utc::now())
            .fetch_all(pool)
            .await
        })
    }

    pub(super) async fn indexed_message(
        &self,
        message_id: Uuid,
    ) -> Result<Option<IndexedMessage>, sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query_as(
                "SELECT id, room_id, content FROM messages \
                 WHERE id = $1 AND recalled_at IS NULL AND trim(content) <> ''",
            )
            .bind(message_id)
            .fetch_optional(pool)
            .await
        })
    }

    pub(super) async fn complete_index_job(&self, job: &IndexJob) -> Result<(), sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query(
                "DELETE FROM message_index_outbox WHERE message_id = $1 AND generation = $2",
            )
            .bind(job.message_id)
            .bind(job.generation)
            .execute(pool)
            .await
            .map(|_| ())
        })
    }

    pub(super) async fn retry_index_job(
        &self,
        job: &IndexJob,
        error: &str,
    ) -> Result<(), sqlx::Error> {
        let exponent = u32::try_from(job.attempt_count.clamp(0, 8)).unwrap_or(8);
        let next_attempt_at = Utc::now() + Duration::seconds(2_i64.pow(exponent).clamp(1, 300));
        let last_error: String = error.chars().take(300).collect();
        with_pool!(self, |pool| {
            sqlx::query(
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
            .map(|_| ())
        })
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
        "SELECT messages.id, messages.sender, messages.content, messages.created_at \
         FROM messages WHERE messages.room_id = ",
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
mod tests {
    use chrono::Utc;

    use super::*;
    use crate::models::Room;

    #[tokio::test]
    async fn retrieved_messages_are_rechecked_against_membership_and_recall_state() {
        let state = AppState::new().await.unwrap();
        let owner = state.insert_user("index-owner", "unused").await.unwrap();
        let outsider = state.insert_user("index-outsider", "unused").await.unwrap();
        let room = Room {
            id: Uuid::new_v4(),
            name: "indexed room".into(),
            password_hash: String::new(),
            has_password: false,
            creator_user_id: Some(owner.id),
            join_policy: "open".into(),
            avatar_emoji: String::new(),
            description: String::new(),
            membership_status: None,
            membership_role: None,
            unread_count: 0,
            created_at: Utc::now(),
        };
        state
            .create_room_with_owner(room.clone(), owner.id)
            .await
            .unwrap();
        let stored = state
            .store_message(
                room.id,
                owner.id,
                &owner.username,
                "",
                "private release plan",
                None,
                Some(Uuid::new_v4()),
            )
            .await
            .unwrap()
            .message;
        let newer = state
            .store_message(
                room.id,
                owner.id,
                &owner.username,
                "",
                "newer indexed detail",
                None,
                Some(Uuid::new_v4()),
            )
            .await
            .unwrap()
            .message;

        let visible = state
            .authorized_retrieved_messages(owner.id, room.id, &[newer.id, stored.id])
            .await
            .unwrap();
        assert_eq!(
            visible.iter().map(|message| message.id).collect::<Vec<_>>(),
            [newer.id, stored.id]
        );
        assert!(state
            .authorized_retrieved_messages(outsider.id, room.id, &[stored.id])
            .await
            .unwrap()
            .is_empty());

        state
            .recall_message(room.id, owner.id, stored.id)
            .await
            .unwrap();
        assert!(state
            .authorized_retrieved_messages(owner.id, room.id, &[stored.id])
            .await
            .unwrap()
            .is_empty());
    }
}
