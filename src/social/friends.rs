use uuid::Uuid;

use crate::social::canonical_pair;
use crate::social::models::SocialUser;
use crate::state::{with_pool, AppState};

impl AppState {
    pub async fn friend_users(&self, user_id: Uuid) -> Result<Vec<SocialUser>, sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query_as(
                "SELECT users.id, users.username, users.avatar_emoji, users.display_name, \
                 users.signature, 'friend' AS relationship \
                 FROM friendships JOIN users ON users.id = CASE \
                   WHEN friendships.user_low_id = $1 THEN friendships.user_high_id \
                   ELSE friendships.user_low_id END \
                 WHERE friendships.status = 'accepted' AND \
                   (friendships.user_low_id = $1 OR friendships.user_high_id = $1) \
                 ORDER BY LOWER(COALESCE(NULLIF(users.display_name, ''), users.username)), users.id",
            )
            .bind(user_id)
            .fetch_all(pool)
            .await
        })
    }

    pub async fn cancel_friend_request(
        &self,
        user_id: Uuid,
        target_id: Uuid,
    ) -> Result<bool, sqlx::Error> {
        let (low, high) = canonical_pair(user_id, target_id);
        with_pool!(self, |pool| {
            sqlx::query(
                "DELETE FROM friendships WHERE user_low_id = $1 AND user_high_id = $2 \
                 AND status = 'pending' AND requested_by_id = $3",
            )
            .bind(low)
            .bind(high)
            .bind(user_id)
            .execute(pool)
            .await
            .map(|result| result.rows_affected() > 0)
        })
    }

    pub async fn remove_friend(
        &self,
        user_id: Uuid,
        target_id: Uuid,
    ) -> Result<Option<Uuid>, sqlx::Error> {
        let (low, high) = canonical_pair(user_id, target_id);
        with_pool!(self, |pool| {
            let mut transaction = pool.begin().await?;
            let removed = sqlx::query(
                "DELETE FROM friendships WHERE user_low_id = $1 AND user_high_id = $2 \
                 AND status = 'accepted'",
            )
            .bind(low)
            .bind(high)
            .execute(&mut *transaction)
            .await?
            .rows_affected();
            if removed == 0 {
                transaction.commit().await?;
                return Ok::<_, sqlx::Error>(None);
            }
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
                .bind(chrono::Utc::now())
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
}
