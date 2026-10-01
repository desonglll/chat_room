//! Chat update and soft-deletion lifecycle.

use chrono::Utc;
use uuid::Uuid;

use crate::{
    models::Chat,
    state::{with_pool, AppState},
};

impl AppState {
    /// Persist a chat edit only if the caller's view is still current.
    pub async fn update_chat(&self, previous: &Chat, updated: Chat) -> Result<bool, sqlx::Error> {
        let changed = with_pool!(self, |pool| {
            sqlx::query(
                "UPDATE chats SET title = $1, password_hash = $2, join_policy = $3, \
                 avatar_emoji = $4, description = $5 \
                 WHERE id = $6 AND title = $7 AND password_hash = $8 AND join_policy = $9",
            )
            .bind(&updated.title)
            .bind(&updated.password_hash)
            .bind(&updated.join_policy)
            .bind(&updated.avatar_emoji)
            .bind(&updated.description)
            .bind(previous.id)
            .bind(&previous.title)
            .bind(&previous.password_hash)
            .bind(&previous.join_policy)
            .execute(pool)
            .await
            .map(|result| result.rows_affected())
        })?;
        if changed == 0 {
            return Ok(false);
        }
        self.cache_updated_chat(updated).await;
        Ok(true)
    }

    /// Soft deletion keeps messages and attachment references recoverable for
    /// an explicit administrator retention/purge workflow.
    pub async fn delete_chat(
        &self,
        id: Uuid,
        expected_password_hash: &str,
    ) -> Result<bool, sqlx::Error> {
        let changed = with_pool!(self, |pool| {
            sqlx::query(
                "UPDATE chats SET deleted_at = $1 \
                 WHERE id = $2 AND password_hash = $3 AND deleted_at IS NULL",
            )
            .bind(Utc::now())
            .bind(id)
            .bind(expected_password_hash)
            .execute(pool)
            .await
            .map(|result| result.rows_affected())
        })?;
        if changed == 0 {
            return Ok(false);
        }
        // Frozen wire value (released clients key their selection cleanup + deletion toast on it).
        self.remove_cached_chat(id, "room deleted").await;
        Ok(true)
    }
}
