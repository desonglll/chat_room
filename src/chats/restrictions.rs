//! Per-member restrictions: step 4 of the authorization decision.
//!
//! A restriction denies one member-scope key to one account until a moment (or for good).
//! It is read live by `authorize_chat_action`, which ignores an expired row, so a restriction
//! lifts at its `until` without waiting for anything. The background sweeper only deletes the
//! expired rows and tells the chat's connections that the member changed.

use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::models::ChatMessage;
use crate::state::{with_pool, AppState};

/// How often the sweeper deletes expired restrictions.
pub const SWEEP_INTERVAL: Duration = Duration::from_secs(30);

/// One denied key, as the member entry and the restriction editor show it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, FromRow, ToSchema)]
pub struct MemberRestriction {
    pub permission_key: String,
    /// `None` = until lifted by hand.
    pub until: Option<DateTime<Utc>>,
}

impl AppState {
    /// Replace an account's restrictions in one chat with `denied` until `until`.
    /// An empty `denied` lifts every restriction.
    pub async fn replace_member_restrictions(
        &self,
        room_id: Uuid,
        user_id: Uuid,
        denied: &[&str],
        until: Option<DateTime<Utc>>,
        restricted_by: Uuid,
    ) -> Result<(), sqlx::Error> {
        let now = Utc::now();
        with_pool!(self, |pool| {
            let mut transaction = pool.begin().await?;
            sqlx::query("DELETE FROM chat_member_restrictions WHERE room_id = $1 AND user_id = $2")
                .bind(room_id)
                .bind(user_id)
                .execute(&mut *transaction)
                .await?;
            for key in denied {
                sqlx::query(
                    "INSERT INTO chat_member_restrictions \
                     (room_id, user_id, denied_permission_key, until, restricted_by, created_at) \
                     VALUES ($1, $2, $3, $4, $5, $6)",
                )
                .bind(room_id)
                .bind(user_id)
                .bind(*key)
                .bind(until)
                .bind(restricted_by)
                .bind(now)
                .execute(&mut *transaction)
                .await?;
            }
            transaction.commit().await
        })
    }

    /// Drop every restriction of one account (appointing an administrator does this).
    pub async fn lift_member_restrictions(
        &self,
        room_id: Uuid,
        user_id: Uuid,
    ) -> Result<(), sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query("DELETE FROM chat_member_restrictions WHERE room_id = $1 AND user_id = $2")
                .bind(room_id)
                .bind(user_id)
                .execute(pool)
                .await
                .map(|_| ())
        })
    }

    /// The restrictions of one account that are still in force at `now`.
    pub async fn member_restrictions(
        &self,
        room_id: Uuid,
        user_id: Uuid,
        now: DateTime<Utc>,
    ) -> Result<Vec<MemberRestriction>, sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query_as(
                "SELECT denied_permission_key AS permission_key, until \
                 FROM chat_member_restrictions \
                 WHERE room_id = $1 AND user_id = $2 AND (until IS NULL OR until > $3) \
                 ORDER BY denied_permission_key",
            )
            .bind(room_id)
            .bind(user_id)
            .bind(now)
            .fetch_all(pool)
            .await
        })
    }

    /// Delete every restriction that expired at or before `now` and report whom it affected.
    /// `now` is a parameter so a test can move the clock instead of sleeping.
    pub async fn purge_expired_chat_restrictions(
        &self,
        now: DateTime<Utc>,
    ) -> Result<Vec<(Uuid, Uuid)>, sqlx::Error> {
        with_pool!(self, |pool| {
            let mut transaction = pool.begin().await?;
            let affected: Vec<(Uuid, Uuid)> = sqlx::query_as(
                "SELECT DISTINCT room_id, user_id FROM chat_member_restrictions \
                 WHERE until IS NOT NULL AND until <= $1",
            )
            .bind(now)
            .fetch_all(&mut *transaction)
            .await?;
            sqlx::query(
                "DELETE FROM chat_member_restrictions WHERE until IS NOT NULL AND until <= $1",
            )
            .bind(now)
            .execute(&mut *transaction)
            .await?;
            transaction.commit().await?;
            Ok(affected)
        })
    }

    /// One sweep: purge, then announce each affected member with a `member_updated` frame so
    /// open clients re-enable what was restricted.
    pub async fn sweep_expired_chat_restrictions(
        &self,
        now: DateTime<Utc>,
    ) -> Result<usize, sqlx::Error> {
        let affected = self.purge_expired_chat_restrictions(now).await?;
        for (room_id, user_id) in &affected {
            self.announce_member(*room_id, *user_id).await?;
        }
        Ok(affected.len())
    }

    /// Broadcast the member's current membership row, if they still have one.
    pub(crate) async fn announce_member(
        &self,
        room_id: Uuid,
        user_id: Uuid,
    ) -> Result<(), sqlx::Error> {
        if let Some(member) = self.chat_membership(room_id, user_id).await? {
            self.broadcast(room_id, ChatMessage::MemberUpdated { member })
                .await;
        }
        Ok(())
    }
}

/// States that already have a sweeper. Weak so a dropped state (every test builds its own)
/// ends its sweeper instead of being kept alive by it.
static SWEEPERS: Mutex<Vec<Weak<AppState>>> = Mutex::new(Vec::new());

/// Start the restriction sweeper for this state once. Called when the router is built.
pub fn ensure_restriction_sweeper(state: Arc<AppState>) {
    {
        let mut sweepers = SWEEPERS
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        sweepers.retain(|existing| existing.strong_count() > 0);
        if sweepers
            .iter()
            .any(|existing| std::ptr::eq(existing.as_ptr(), Arc::as_ptr(&state)))
        {
            return;
        }
        sweepers.push(Arc::downgrade(&state));
    }
    let weak = Arc::downgrade(&state);
    drop(state);
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(SWEEP_INTERVAL).await;
            let Some(state) = weak.upgrade() else {
                return;
            };
            if let Err(error) = state.sweep_expired_chat_restrictions(Utc::now()).await {
                tracing::warn!("sweep expired chat restrictions failed: {error}");
            }
        }
    });
}
