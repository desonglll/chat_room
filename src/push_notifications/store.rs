use chrono::{DateTime, Duration, Utc};
use uuid::Uuid;

use super::models::{ClaimedPushJob, PushSubscriptionView, SavePushSubscriptionRequest};
use crate::state::{with_pool, AppState};

impl AppState {
    pub(crate) async fn save_push_subscription(
        &self,
        user_id: Uuid,
        request: &SavePushSubscriptionRequest,
    ) -> Result<Option<PushSubscriptionView>, sqlx::Error> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now();
        with_pool!(self, |pool| {
            sqlx::query_as(
                "INSERT INTO push_subscriptions \
                 (id, user_id, endpoint, p256dh, auth, show_details, created_at, updated_at) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $7) \
                 ON CONFLICT(endpoint) DO UPDATE SET \
                   user_id = excluded.user_id, p256dh = excluded.p256dh, auth = excluded.auth, \
                   show_details = excluded.show_details, updated_at = excluded.updated_at \
                 WHERE push_subscriptions.user_id = excluded.user_id OR \
                   (push_subscriptions.p256dh = excluded.p256dh AND \
                    push_subscriptions.auth = excluded.auth) \
                 RETURNING id, show_details, created_at, updated_at",
            )
            .bind(id)
            .bind(user_id)
            .bind(&request.endpoint)
            .bind(&request.keys.p256dh)
            .bind(&request.keys.auth)
            .bind(request.show_details)
            .bind(now)
            .fetch_optional(pool)
            .await
        })
    }

    pub(crate) async fn delete_push_subscription(
        &self,
        user_id: Uuid,
        endpoint: &str,
    ) -> Result<bool, sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query("DELETE FROM push_subscriptions WHERE user_id = $1 AND endpoint = $2")
                .bind(user_id)
                .bind(endpoint)
                .execute(pool)
                .await
                .map(|result| result.rows_affected() > 0)
        })
    }

    pub(crate) async fn claim_push_jobs(
        &self,
        limit: i64,
    ) -> Result<(String, Vec<ClaimedPushJob>), sqlx::Error> {
        let now = Utc::now();
        let stale_before = now - Duration::minutes(2);
        let claim_token = Uuid::new_v4().to_string();
        with_pool!(self, |pool| {
            sqlx::query(
                "UPDATE push_delivery_jobs SET claimed_at = $1, claim_token = $2 \
                 WHERE id IN (SELECT id FROM push_delivery_jobs \
                   WHERE next_attempt_at <= $1 AND (claimed_at IS NULL OR claimed_at < $3) \
                   ORDER BY next_attempt_at, created_at LIMIT $4) \
                 AND (claimed_at IS NULL OR claimed_at < $3)",
            )
            .bind(now)
            .bind(&claim_token)
            .bind(stale_before)
            .bind(limit)
            .execute(pool)
            .await
            .map(|_| ())
        })?;
        let jobs = with_pool!(self, |pool| {
            sqlx::query_as(
                "SELECT job.id, job.notification_id, job.subscription_id, \
                   notification.recipient_id, subscription.endpoint, subscription.p256dh, \
                   subscription.auth, subscription.show_details, job.attempts \
                 FROM push_delivery_jobs AS job \
                 JOIN notifications AS notification ON notification.id = job.notification_id \
                 JOIN push_subscriptions AS subscription ON subscription.id = job.subscription_id \
                 WHERE job.claim_token = $1 ORDER BY job.created_at",
            )
            .bind(&claim_token)
            .fetch_all(pool)
            .await
        })?;
        Ok((claim_token, jobs))
    }

    pub(crate) async fn complete_push_job(
        &self,
        job_id: &str,
        claim_token: &str,
    ) -> Result<(), sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query("DELETE FROM push_delivery_jobs WHERE id = $1 AND claim_token = $2")
                .bind(job_id)
                .bind(claim_token)
                .execute(pool)
                .await
                .map(|_| ())
        })
    }

    pub(crate) async fn expire_push_subscription(
        &self,
        subscription_id: &str,
    ) -> Result<(), sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query("DELETE FROM push_subscriptions WHERE id = $1")
                .bind(subscription_id)
                .execute(pool)
                .await
                .map(|_| ())
        })
    }

    pub(crate) async fn retry_push_job(
        &self,
        job: &ClaimedPushJob,
        claim_token: &str,
        max_attempts: i32,
    ) -> Result<(), sqlx::Error> {
        let attempts = job.attempts.saturating_add(1);
        if attempts >= max_attempts {
            return self.complete_push_job(&job.id, claim_token).await;
        }
        let delay_seconds = 2_i64.saturating_pow(attempts as u32).min(300);
        let next_attempt_at = Utc::now() + Duration::seconds(delay_seconds);
        with_pool!(self, |pool| {
            sqlx::query(
                "UPDATE push_delivery_jobs SET attempts = $1, next_attempt_at = $2, \
                   claimed_at = NULL, claim_token = NULL WHERE id = $3 AND claim_token = $4",
            )
            .bind(attempts)
            .bind(next_attempt_at)
            .bind(&job.id)
            .bind(claim_token)
            .execute(pool)
            .await
            .map(|_| ())
        })
    }

    pub(crate) async fn room_allows_push(
        &self,
        recipient_id: Uuid,
        room_id: Uuid,
        mentions_only_event: bool,
    ) -> Result<bool, sqlx::Error> {
        let preference: Option<(String, Option<DateTime<Utc>>)> = with_pool!(self, |pool| {
            sqlx::query_as(
                "SELECT notification_level, muted_until FROM room_memberships \
                 WHERE user_id = $1 AND room_id = $2 AND status = 'active'",
            )
            .bind(recipient_id)
            .bind(room_id)
            .fetch_optional(pool)
            .await
        })?;
        let Some((level, muted_until)) = preference else {
            return Ok(false);
        };
        if muted_until.is_some_and(|until| until > Utc::now()) || level == "none" {
            return Ok(false);
        }
        Ok(level == "all" || (level == "mentions" && mentions_only_event))
    }
}
