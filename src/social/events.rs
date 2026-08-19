use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::state::{with_pool, AppState};

#[derive(Clone, Eq, PartialEq, sqlx::FromRow)]
pub(crate) struct SocialFingerprintEntry {
    other_user_id: Uuid,
    state: String,
    updated_at: DateTime<Utc>,
}

pub(crate) struct SocialAccountState {
    pub fingerprint: Vec<SocialFingerprintEntry>,
    pub incoming_request_count: usize,
}

impl AppState {
    pub(crate) async fn social_account_state(
        &self,
        user_id: Uuid,
    ) -> Result<SocialAccountState, sqlx::Error> {
        let fingerprint: Vec<SocialFingerprintEntry> = with_pool!(self, |pool| {
            sqlx::query_as(
                "SELECT CASE WHEN user_low_id = $1 THEN user_high_id ELSE user_low_id END \
                   AS other_user_id, CASE WHEN status = 'accepted' THEN 'friend' \
                   WHEN requested_by_id = $1 THEN 'outgoing' ELSE 'incoming' END AS state, \
                   updated_at FROM friendships \
                 WHERE user_low_id = $1 OR user_high_id = $1 \
                 UNION ALL \
                 SELECT blocked_id AS other_user_id, 'blocked' AS state, created_at AS updated_at \
                 FROM user_blocks WHERE blocker_id = $1 \
                 ORDER BY state, other_user_id",
            )
            .bind(user_id)
            .fetch_all(pool)
            .await
        })?;
        let incoming_request_count = fingerprint
            .iter()
            .filter(|entry| entry.state == "incoming")
            .count();
        Ok(SocialAccountState {
            fingerprint,
            incoming_request_count,
        })
    }
}
