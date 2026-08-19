use chrono::Utc;
use uuid::Uuid;

use crate::social::canonical_pair;
use crate::social::models::SocialUser;
use crate::state::{with_pool, AppState};

impl AppState {
    pub async fn blocked_users(&self, user_id: Uuid) -> Result<Vec<SocialUser>, sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query_as(
                "SELECT users.id, users.username, users.avatar_emoji, users.display_name, \
                 users.signature, 'blocked' AS relationship \
                 FROM user_blocks JOIN users ON users.id = user_blocks.blocked_id \
                 WHERE user_blocks.blocker_id = $1 \
                 ORDER BY user_blocks.created_at DESC, users.id",
            )
            .bind(user_id)
            .fetch_all(pool)
            .await
        })
    }

    pub async fn block_user(
        &self,
        user_id: Uuid,
        target_id: Uuid,
    ) -> Result<Option<Uuid>, sqlx::Error> {
        let (low, high) = canonical_pair(user_id, target_id);
        with_pool!(self, |pool| {
            let mut transaction = pool.begin().await?;
            let target_exists: bool =
                sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users WHERE id = $1)")
                    .bind(target_id)
                    .fetch_one(&mut *transaction)
                    .await?;
            if !target_exists {
                return Err(sqlx::Error::RowNotFound);
            }
            sqlx::query(
                "INSERT INTO user_blocks (blocker_id, blocked_id, created_at) \
                 VALUES ($1, $2, $3) ON CONFLICT(blocker_id, blocked_id) DO NOTHING",
            )
            .bind(user_id)
            .bind(target_id)
            .bind(Utc::now())
            .execute(&mut *transaction)
            .await?;
            sqlx::query("DELETE FROM friendships WHERE user_low_id = $1 AND user_high_id = $2")
                .bind(low)
                .bind(high)
                .execute(&mut *transaction)
                .await?;
            let room_id: Option<Uuid> = sqlx::query_scalar(
                "SELECT room_id FROM direct_conversations \
                 WHERE user_low_id = $1 AND user_high_id = $2",
            )
            .bind(low)
            .bind(high)
            .fetch_optional(&mut *transaction)
            .await?;
            if let Some(room_id) = room_id {
                sqlx::query(
                    "DELETE FROM room_memberships WHERE room_id = $1 \
                     AND (user_id = $2 OR user_id = $3)",
                )
                .bind(room_id)
                .bind(low)
                .bind(high)
                .execute(&mut *transaction)
                .await?;
                sqlx::query(
                    "UPDATE attachment_uploads SET status = 'aborted', updated_at = $1 \
                     WHERE room_id = $2 AND status = 'in_progress' \
                     AND (uploader_id = $3 OR uploader_id = $4)",
                )
                .bind(Utc::now())
                .bind(room_id)
                .bind(low)
                .bind(high)
                .execute(&mut *transaction)
                .await?;
            }
            transaction.commit().await?;
            Ok::<_, sqlx::Error>(room_id)
        })
    }

    pub async fn unblock_user(&self, user_id: Uuid, target_id: Uuid) -> Result<(), sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query("DELETE FROM user_blocks WHERE blocker_id = $1 AND blocked_id = $2")
                .bind(user_id)
                .bind(target_id)
                .execute(pool)
                .await
                .map(|_| ())
        })
    }
}
