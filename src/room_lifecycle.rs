//! Room update and soft-deletion lifecycle.

use chrono::Utc;
use uuid::Uuid;

use crate::{
    models::Room,
    state::{with_pool, AppState},
};

impl AppState {
    /// Persist a room edit only if the caller's view is still current.
    pub async fn update_room(&self, previous: &Room, updated: Room) -> Result<bool, sqlx::Error> {
        let changed = with_pool!(self, |pool| {
            sqlx::query(
                "UPDATE rooms SET name = $1, password_hash = $2, join_policy = $3, \
                 avatar_emoji = $4, description = $5 \
                 WHERE id = $6 AND name = $7 AND password_hash = $8 AND join_policy = $9",
            )
            .bind(&updated.name)
            .bind(&updated.password_hash)
            .bind(&updated.join_policy)
            .bind(&updated.avatar_emoji)
            .bind(&updated.description)
            .bind(previous.id)
            .bind(&previous.name)
            .bind(&previous.password_hash)
            .bind(&previous.join_policy)
            .execute(pool)
            .await
            .map(|result| result.rows_affected())
        })?;
        if changed == 0 {
            return Ok(false);
        }
        self.cache_updated_room(updated).await;
        Ok(true)
    }

    /// Soft deletion keeps messages and attachment references recoverable for
    /// an explicit administrator retention/purge workflow.
    pub async fn delete_room(
        &self,
        id: Uuid,
        expected_password_hash: &str,
    ) -> Result<bool, sqlx::Error> {
        let changed = with_pool!(self, |pool| {
            sqlx::query(
                "UPDATE rooms SET deleted_at = $1 \
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
        self.remove_cached_room(id, "room deleted").await;
        Ok(true)
    }
}
