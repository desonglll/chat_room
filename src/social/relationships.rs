use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::models::UserSummary;
use crate::social::models::{FriendRequestOutcome, FriendRequestView, SocialUser};
use crate::state::{with_pool, AppState};

pub fn canonical_pair(left: Uuid, right: Uuid) -> (Uuid, Uuid) {
    if left.as_bytes() <= right.as_bytes() {
        (left, right)
    } else {
        (right, left)
    }
}

fn escaped_like(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

impl AppState {
    pub async fn search_social_users(
        &self,
        viewer_id: Uuid,
        query: &str,
        limit: i64,
    ) -> Result<Vec<SocialUser>, sqlx::Error> {
        let pattern = format!("%{}%", escaped_like(query));
        with_pool!(self, |pool| {
            sqlx::query_as(
                "SELECT users.id, users.username, users.avatar_emoji, users.display_name, \
                 users.signature, COALESCE(remarks.remark, '') AS remark, CASE \
                   WHEN friendships.status = 'accepted' THEN 'friend' \
                   WHEN friendships.requested_by_id = $1 THEN 'outgoing' \
                   WHEN friendships.status = 'pending' THEN 'incoming' \
                   ELSE 'none' END AS relationship \
                 FROM users LEFT JOIN friendships ON \
                   (friendships.user_low_id = $1 AND friendships.user_high_id = users.id) OR \
                   (friendships.user_high_id = $1 AND friendships.user_low_id = users.id) \
                 LEFT JOIN friend_remarks AS remarks ON remarks.owner_id = $1 \
                   AND remarks.friend_id = users.id \
                 WHERE users.id <> $1 \
                   AND (LOWER(users.username) LIKE LOWER($3) ESCAPE '\\' \
                     OR LOWER(users.display_name) LIKE LOWER($3) ESCAPE '\\') \
                   AND NOT EXISTS (SELECT 1 FROM user_blocks WHERE \
                     (blocker_id = $1 AND blocked_id = users.id) OR \
                     (blocker_id = users.id AND blocked_id = $1)) \
                 ORDER BY CASE WHEN LOWER(users.username) = LOWER($2) THEN 0 \
                   WHEN LOWER(users.username) LIKE LOWER($2) || '%' THEN 1 ELSE 2 END, \
                   LOWER(NULLIF(users.display_name, '')), LOWER(users.username), users.id \
                 LIMIT $4",
            )
            .bind(viewer_id)
            .bind(query)
            .bind(pattern)
            .bind(limit)
            .fetch_all(pool)
            .await
        })
    }

    pub(crate) async fn send_friend_request(
        &self,
        requester_id: Uuid,
        target_id: Uuid,
    ) -> Result<FriendRequestOutcome, sqlx::Error> {
        let (low, high) = canonical_pair(requester_id, target_id);
        let now = Utc::now();
        with_pool!(self, |pool| {
            let mut transaction = pool.begin().await?;
            let blocked: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM user_blocks WHERE \
                 (blocker_id = $1 AND blocked_id = $2) OR \
                 (blocker_id = $2 AND blocked_id = $1))",
            )
            .bind(requester_id)
            .bind(target_id)
            .fetch_one(&mut *transaction)
            .await?;
            if blocked {
                return Err(sqlx::Error::RowNotFound);
            }
            let target_exists: bool =
                sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users WHERE id = $1)")
                    .bind(target_id)
                    .fetch_one(&mut *transaction)
                    .await?;
            if !target_exists {
                return Err(sqlx::Error::RowNotFound);
            }
            let existing: Option<(String, Uuid)> = sqlx::query_as(
                "SELECT status, requested_by_id FROM friendships \
                 WHERE user_low_id = $1 AND user_high_id = $2",
            )
            .bind(low)
            .bind(high)
            .fetch_optional(&mut *transaction)
            .await?;
            let outcome = match existing {
                Some((status, _)) if status == "accepted" => FriendRequestOutcome::Accepted,
                Some((_, requested_by)) if requested_by == requester_id => {
                    FriendRequestOutcome::Pending
                }
                Some(_) => {
                    sqlx::query(
                        "UPDATE friendships SET status = 'accepted', accepted_at = $1, \
                         updated_at = $2 WHERE user_low_id = $3 AND user_high_id = $4",
                    )
                    .bind(now)
                    .bind(now)
                    .bind(low)
                    .bind(high)
                    .execute(&mut *transaction)
                    .await?;
                    self.social_rate_limits
                        .clear_pair_request((low, high))
                        .await;
                    FriendRequestOutcome::Accepted
                }
                None => {
                    if !self
                        .social_rate_limits
                        .allow_new_pair_request((requester_id, target_id))
                        .await
                    {
                        transaction.commit().await?;
                        return Ok::<_, sqlx::Error>(FriendRequestOutcome::RateLimited);
                    }
                    sqlx::query(
                        "INSERT INTO friendships (user_low_id, user_high_id, requested_by_id, \
                         status, created_at, updated_at) VALUES ($1, $2, $3, 'pending', $4, $5)",
                    )
                    .bind(low)
                    .bind(high)
                    .bind(requester_id)
                    .bind(now)
                    .bind(now)
                    .execute(&mut *transaction)
                    .await?;
                    FriendRequestOutcome::Created
                }
            };
            transaction.commit().await?;
            Ok::<_, sqlx::Error>(outcome)
        })
    }

    pub async fn friend_requests(
        &self,
        user_id: Uuid,
        direction: &str,
    ) -> Result<Vec<FriendRequestView>, sqlx::Error> {
        #[derive(sqlx::FromRow)]
        struct Row {
            id: Uuid,
            username: String,
            avatar_emoji: String,
            display_name: String,
            direction: String,
            created_at: DateTime<Utc>,
        }

        let rows: Vec<Row> = with_pool!(self, |pool| {
            sqlx::query_as(
                "SELECT users.id, users.username, users.avatar_emoji, users.display_name, \
                 CASE WHEN friendships.requested_by_id = $1 THEN 'outgoing' \
                   ELSE 'incoming' END AS direction, friendships.created_at \
                 FROM friendships JOIN users ON users.id = CASE \
                   WHEN friendships.user_low_id = $1 THEN friendships.user_high_id \
                   ELSE friendships.user_low_id END \
                 WHERE friendships.status = 'pending' \
                   AND (friendships.user_low_id = $1 OR friendships.user_high_id = $1) \
                   AND (($2 = 'incoming' AND friendships.requested_by_id <> $1) \
                     OR ($2 = 'outgoing' AND friendships.requested_by_id = $1)) \
                 ORDER BY friendships.created_at DESC, users.id",
            )
            .bind(user_id)
            .bind(direction)
            .fetch_all(pool)
            .await
        })?;
        Ok(rows
            .into_iter()
            .map(|row| FriendRequestView {
                user: UserSummary {
                    id: row.id,
                    username: row.username,
                    avatar_emoji: row.avatar_emoji,
                    display_name: row.display_name,
                },
                direction: row.direction,
                created_at: row.created_at,
            })
            .collect())
    }

    pub async fn respond_friend_request(
        &self,
        user_id: Uuid,
        requester_id: Uuid,
        accept: bool,
    ) -> Result<bool, sqlx::Error> {
        let (low, high) = canonical_pair(user_id, requester_id);
        let now = Utc::now();
        let changed = with_pool!(self, |pool| {
            let result = if accept {
                sqlx::query(
                    "UPDATE friendships SET status = 'accepted', accepted_at = $1, \
                     updated_at = $2 WHERE user_low_id = $3 AND user_high_id = $4 \
                     AND status = 'pending' AND requested_by_id = $5",
                )
                .bind(now)
                .bind(now)
                .bind(low)
                .bind(high)
                .bind(requester_id)
                .execute(pool)
                .await?
            } else {
                sqlx::query(
                    "DELETE FROM friendships WHERE user_low_id = $1 AND user_high_id = $2 \
                     AND status = 'pending' AND requested_by_id = $3",
                )
                .bind(low)
                .bind(high)
                .bind(requester_id)
                .execute(pool)
                .await?
            };
            Ok::<_, sqlx::Error>(result.rows_affected() > 0)
        })?;
        if accept && changed {
            self.social_rate_limits
                .clear_pair_request((low, high))
                .await;
        }
        Ok(changed)
    }

    pub async fn are_friends(&self, left: Uuid, right: Uuid) -> Result<bool, sqlx::Error> {
        let (low, high) = canonical_pair(left, right);
        with_pool!(self, |pool| {
            sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM friendships WHERE user_low_id = $1 \
                 AND user_high_id = $2 AND status = 'accepted')",
            )
            .bind(low)
            .bind(high)
            .fetch_one(pool)
            .await
        })
    }
}
