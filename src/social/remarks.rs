use chrono::Utc;
use uuid::Uuid;

use crate::social::canonical_pair;
use crate::state::{with_pool, AppState};

impl AppState {
    pub async fn set_friend_remark(
        &self,
        owner_id: Uuid,
        friend_id: Uuid,
        remark: &str,
    ) -> Result<bool, sqlx::Error> {
        let (low, high) = canonical_pair(owner_id, friend_id);
        with_pool!(self, |pool| {
            let friends: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM friendships WHERE user_low_id = $1 \
                 AND user_high_id = $2 AND status = 'accepted')",
            )
            .bind(low)
            .bind(high)
            .fetch_one(pool)
            .await?;
            if !friends {
                return Ok::<_, sqlx::Error>(false);
            }
            if remark.is_empty() {
                sqlx::query("DELETE FROM friend_remarks WHERE owner_id = $1 AND friend_id = $2")
                    .bind(owner_id)
                    .bind(friend_id)
                    .execute(pool)
                    .await?;
            } else {
                sqlx::query(
                    "INSERT INTO friend_remarks (owner_id, friend_id, remark, updated_at) \
                     VALUES ($1, $2, $3, $4) ON CONFLICT(owner_id, friend_id) DO UPDATE SET \
                     remark = excluded.remark, updated_at = excluded.updated_at",
                )
                .bind(owner_id)
                .bind(friend_id)
                .bind(remark)
                .bind(Utc::now())
                .execute(pool)
                .await?;
            }
            Ok::<_, sqlx::Error>(true)
        })
    }
}
