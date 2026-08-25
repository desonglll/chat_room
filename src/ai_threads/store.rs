use chrono::Utc;
use uuid::Uuid;

use super::models::{AiThread, AiThreadMessage};
use crate::state::{with_pool, AppState};

impl AppState {
    pub async fn ai_threads(&self, user_id: Uuid) -> Result<Vec<AiThread>, sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query_as(
                "SELECT id, user_id, title, room_id, thinking_enabled, created_at, updated_at \
                 FROM ai_threads WHERE user_id = $1 ORDER BY updated_at DESC, id DESC LIMIT 200",
            )
            .bind(user_id)
            .fetch_all(pool)
            .await
        })
    }

    pub async fn ai_thread(
        &self,
        user_id: Uuid,
        thread_id: Uuid,
    ) -> Result<Option<AiThread>, sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query_as(
                "SELECT id, user_id, title, room_id, thinking_enabled, created_at, updated_at \
                 FROM ai_threads WHERE id = $1 AND user_id = $2",
            )
            .bind(thread_id)
            .bind(user_id)
            .fetch_optional(pool)
            .await
        })
    }

    pub async fn create_ai_thread(
        &self,
        user_id: Uuid,
        title: &str,
        room_id: Option<Uuid>,
        thinking_enabled: bool,
    ) -> Result<AiThread, sqlx::Error> {
        let id = Uuid::new_v4();
        let now = Utc::now();
        with_pool!(self, |pool| {
            sqlx::query(
                "INSERT INTO ai_threads \
                 (id, user_id, title, room_id, thinking_enabled, created_at, updated_at) \
                 VALUES ($1, $2, $3, $4, $5, $6, $6)",
            )
            .bind(id)
            .bind(user_id)
            .bind(title)
            .bind(room_id)
            .bind(thinking_enabled)
            .bind(now)
            .execute(pool)
            .await
            .map(|_| ())
        })?;
        self.ai_thread(user_id, id)
            .await?
            .ok_or(sqlx::Error::RowNotFound)
    }

    pub async fn update_ai_thread(
        &self,
        user_id: Uuid,
        thread_id: Uuid,
        title: &str,
        room_id: Option<Uuid>,
        thinking_enabled: bool,
    ) -> Result<Option<AiThread>, sqlx::Error> {
        let now = Utc::now();
        let changed = with_pool!(self, |pool| {
            sqlx::query(
                "UPDATE ai_threads SET title = $1, room_id = $2, thinking_enabled = $3, \
                 updated_at = $4 WHERE id = $5 AND user_id = $6",
            )
            .bind(title)
            .bind(room_id)
            .bind(thinking_enabled)
            .bind(now)
            .bind(thread_id)
            .bind(user_id)
            .execute(pool)
            .await
            .map(|result| result.rows_affected() > 0)
        })?;
        if !changed {
            return Ok(None);
        }
        self.ai_thread(user_id, thread_id).await
    }

    pub async fn delete_ai_thread(
        &self,
        user_id: Uuid,
        thread_id: Uuid,
    ) -> Result<bool, sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query("DELETE FROM ai_threads WHERE id = $1 AND user_id = $2")
                .bind(thread_id)
                .bind(user_id)
                .execute(pool)
                .await
                .map(|result| result.rows_affected() > 0)
        })
    }

    pub async fn ai_thread_messages(
        &self,
        user_id: Uuid,
        thread_id: Uuid,
        limit: i64,
    ) -> Result<Option<Vec<AiThreadMessage>>, sqlx::Error> {
        if self.ai_thread(user_id, thread_id).await?.is_none() {
            return Ok(None);
        }
        let mut messages: Vec<AiThreadMessage> = with_pool!(self, |pool| {
            sqlx::query_as(
                "SELECT id, thread_id, role, content, room_id, context_message_count, status, \
                   revision, created_at, updated_at \
                 FROM (SELECT id, thread_id, role, content, room_id, context_message_count, \
                   status, revision, created_at, COALESCE(updated_at, created_at) AS updated_at \
                   FROM ai_thread_messages WHERE thread_id = $1 \
                   ORDER BY created_at DESC, id DESC LIMIT $2) AS recent \
                 ORDER BY created_at ASC, id ASC",
            )
            .bind(thread_id)
            .bind(limit)
            .fetch_all(pool)
            .await
        })?;
        if let Some(cache) = self.redis_cache() {
            for message in messages.iter_mut().filter(|message| {
                message.role == "assistant"
                    && matches!(message.status.as_str(), "pending" | "streaming")
            }) {
                match cache.ai_answer(message.id).await {
                    Ok(Some(answer)) => {
                        message.content = answer.content;
                        message.context_message_count = Some(answer.context_message_count);
                        message.status = answer.status;
                        message.revision = answer.revision;
                        message.updated_at = answer.updated_at;
                    }
                    Ok(None) => {}
                    Err(error) => tracing::warn!(
                        message_id = %message.id,
                        "read live AI answer from Redis failed: {error:#}"
                    ),
                }
            }
        }
        Ok(Some(messages))
    }

    pub async fn append_ai_thread_message(
        &self,
        user_id: Uuid,
        thread_id: Uuid,
        role: &str,
        content: &str,
        room_id: Option<Uuid>,
        context_message_count: Option<i64>,
    ) -> Result<Option<AiThreadMessage>, sqlx::Error> {
        let id = Uuid::new_v4();
        let now = Utc::now();
        let inserted = with_pool!(self, |pool| {
            sqlx::query(
                "INSERT INTO ai_thread_messages \
                 (id, thread_id, role, content, room_id, context_message_count, status, revision, created_at, updated_at) \
                 SELECT $1, $2, $3, $4, $5, $6, 'completed', 0, $7, $7 FROM ai_threads \
                 WHERE id = $2 AND user_id = $8",
            )
            .bind(id)
            .bind(thread_id)
            .bind(role)
            .bind(content)
            .bind(room_id)
            .bind(context_message_count)
            .bind(now)
            .bind(user_id)
            .execute(pool)
            .await
            .map(|result| result.rows_affected() > 0)
        })?;
        if !inserted {
            return Ok(None);
        }
        with_pool!(self, |pool| {
            sqlx::query("UPDATE ai_threads SET updated_at = $1 WHERE id = $2")
                .bind(now)
                .bind(thread_id)
                .execute(pool)
                .await
                .map(|_| ())
        })?;
        with_pool!(self, |pool| {
            sqlx::query_as(
                "SELECT id, thread_id, role, content, room_id, context_message_count, status, \
                   revision, created_at, COALESCE(updated_at, created_at) AS updated_at \
                 FROM ai_thread_messages WHERE id = $1",
            )
            .bind(id)
            .fetch_optional(pool)
            .await
        })
    }
}
