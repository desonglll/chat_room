use chrono::{Duration, Utc};
use uuid::Uuid;

use crate::state::{with_pool, AppState};

#[derive(Clone, sqlx::FromRow)]
pub(super) struct GraphJob {
    pub message_id: Uuid,
    pub room_id: Uuid,
    pub operation: String,
    pub attempt_count: i64,
    pub generation: i64,
}

#[derive(sqlx::FromRow)]
pub(super) struct GraphMessage {
    pub id: Uuid,
    pub room_id: Uuid,
    pub sender: String,
    pub content: String,
    pub created_at: chrono::DateTime<Utc>,
}

impl AppState {
    pub(super) async fn ready_graph_jobs(&self) -> Result<Vec<GraphJob>, sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query_as(
                "SELECT message_id, room_id, operation, attempt_count, generation \
                 FROM message_graph_outbox WHERE next_attempt_at <= $1 \
                 ORDER BY updated_at ASC LIMIT 100",
            )
            .bind(Utc::now())
            .fetch_all(pool)
            .await
        })
    }

    pub(super) async fn graph_message(
        &self,
        message_id: Uuid,
    ) -> Result<Option<GraphMessage>, sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query_as(
                "SELECT id, room_id, sender, content, created_at FROM messages \
                 WHERE id = $1 AND recalled_at IS NULL AND trim(content) <> ''",
            )
            .bind(message_id)
            .fetch_optional(pool)
            .await
        })
    }

    pub(super) async fn complete_graph_job(&self, job: &GraphJob) -> Result<(), sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query(
                "DELETE FROM message_graph_outbox WHERE message_id = $1 AND generation = $2",
            )
            .bind(job.message_id)
            .bind(job.generation)
            .execute(pool)
            .await
            .map(|_| ())
        })
    }

    pub(super) async fn retry_graph_job(
        &self,
        job: &GraphJob,
        error: &str,
    ) -> Result<(), sqlx::Error> {
        let exponent = u32::try_from(job.attempt_count.clamp(0, 8)).unwrap_or(8);
        let next_attempt_at = Utc::now() + Duration::seconds(2_i64.pow(exponent).clamp(1, 300));
        let last_error: String = error.chars().take(300).collect();
        with_pool!(self, |pool| {
            sqlx::query(
                "UPDATE message_graph_outbox SET attempt_count = attempt_count + 1, \
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Room;

    #[tokio::test]
    async fn migration_backfills_and_tracks_message_lifecycle_with_room_id() {
        let state = AppState::new().await.unwrap();
        let owner = state.insert_user("graph-owner", "unused").await.unwrap();
        let room = Room {
            id: Uuid::new_v4(),
            name: "graph room".into(),
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
                "Graph this decision",
                None,
                Some(Uuid::new_v4()),
            )
            .await
            .unwrap()
            .message;
        let job: (Uuid, String, i64) = sqlx::query_as(
            "SELECT room_id, operation, generation FROM message_graph_outbox WHERE message_id = ?",
        )
        .bind(stored.id)
        .fetch_one(state.pool())
        .await
        .unwrap();
        assert_eq!(job, (room.id, "upsert".into(), 1));

        sqlx::query("UPDATE messages SET recalled_at = ? WHERE id = ?")
            .bind(Utc::now())
            .bind(stored.id)
            .execute(state.pool())
            .await
            .unwrap();
        let job: (String, i64) = sqlx::query_as(
            "SELECT operation, generation FROM message_graph_outbox WHERE message_id = ?",
        )
        .bind(stored.id)
        .fetch_one(state.pool())
        .await
        .unwrap();
        assert_eq!(job, ("delete".into(), 2));
    }
}
