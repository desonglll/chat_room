use chrono::Utc;
use uuid::Uuid;

use super::RoomAiPolicy;
use crate::state::{with_pool, AppState};

const NEW_RUNS_ONLY: &str = "new_runs_only";

impl AppState {
    pub async fn room_ai_policy(&self, room_id: Uuid) -> Result<RoomAiPolicy, sqlx::Error> {
        let row: Option<(String, i64, chrono::DateTime<Utc>)> = with_pool!(self, |pool| {
            sqlx::query_as(
                "SELECT mode, version, updated_at FROM room_ai_policies WHERE room_id = $1",
            )
            .bind(room_id)
            .fetch_optional(pool)
            .await
        })?;
        Ok(match row {
            Some((mode, version, updated_at)) => RoomAiPolicy {
                room_id,
                mode,
                version,
                applies_to: NEW_RUNS_ONLY.into(),
                updated_at: Some(updated_at),
            },
            None => RoomAiPolicy {
                room_id,
                mode: "members".into(),
                version: 0,
                applies_to: NEW_RUNS_ONLY.into(),
                updated_at: None,
            },
        })
    }

    pub async fn update_room_ai_policy(
        &self,
        room_id: Uuid,
        actor_id: Uuid,
        mode: &str,
        expected_version: i64,
    ) -> Result<Option<RoomAiPolicy>, sqlx::Error> {
        let now = Utc::now();
        let changed = with_pool!(self, |pool| {
            sqlx::query(
                "INSERT INTO room_ai_policies (room_id, mode, version, updated_by, updated_at) \
                 SELECT $1, $2, 1, $3, $4 WHERE $5 = 0 \
                 ON CONFLICT (room_id) DO UPDATE SET mode = excluded.mode, \
                   version = room_ai_policies.version + 1, updated_by = excluded.updated_by, \
                   updated_at = excluded.updated_at WHERE room_ai_policies.version = $5",
            )
            .bind(room_id)
            .bind(mode)
            .bind(actor_id)
            .bind(now)
            .bind(expected_version)
            .execute(pool)
            .await
            .map(|result| result.rows_affected() == 1)
        })?;
        if !changed {
            return Ok(None);
        }
        self.room_ai_policy(room_id).await.map(Some)
    }
}
