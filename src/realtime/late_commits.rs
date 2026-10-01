//! TG-604: live delivery must not lose a message that commits late.
//!
//! The poller walks a `(created_at, id)` cursor, but `created_at` is taken before the insert
//! waits for the write lock. Under write contention a message can commit *after* a newer one
//! was delivered, with a timestamp behind the cursor — and the cursor would never return to
//! it (found by the TG-604 mixed stress run: text sends lost while uploads flooded the chat).
//! So every poll also lists the ids in a short trailing window and delivers the ones this
//! connection has not sent yet. Memory is bounded by the window.

use std::collections::{HashSet, VecDeque};

use chrono::{DateTime, Duration, Utc};
use uuid::Uuid;

use crate::models::StoredMessage;
use crate::state::{with_pool, AppState};

/// How far behind the cursor a commit may land and still be delivered live.
pub(super) const LATE_COMMIT_WINDOW_SECS: i64 = 10;

#[derive(Default)]
pub(super) struct LateCommits {
    sent: HashSet<Uuid>,
    order: VecDeque<(DateTime<Utc>, Uuid)>,
    primed: bool,
}

impl LateCommits {
    /// Record a message this connection delivered (by the cursor walk or the late check).
    pub(super) fn sent(&mut self, id: Uuid, created_at: DateTime<Utc>) {
        if self.sent.insert(id) {
            self.order.push_back((created_at, id));
        }
    }

    fn prune(&mut self, horizon: DateTime<Utc>) {
        while self.order.front().is_some_and(|(at, _)| *at < horizon) {
            if let Some((_, id)) = self.order.pop_front() {
                self.sent.remove(&id);
            }
        }
    }

    /// Messages in the trailing window behind `cursor_at` that this connection has not sent.
    /// The first call only learns what is already there (history replay delivered it).
    pub(super) async fn missed(
        &mut self,
        state: &AppState,
        room_id: Uuid,
        cursor_at: DateTime<Utc>,
        viewer_id: Uuid,
    ) -> Result<Vec<StoredMessage>, sqlx::Error> {
        let horizon = cursor_at - Duration::seconds(LATE_COMMIT_WINDOW_SECS);
        self.prune(horizon);
        let rows: Vec<(Uuid, DateTime<Utc>)> = with_pool!(state, |pool| {
            sqlx::query_as(
                "SELECT id, created_at FROM messages \
                 WHERE room_id = $1 AND created_at > $2 AND created_at <= $3 \
                 ORDER BY created_at, id",
            )
            .bind(room_id)
            .bind(horizon)
            .bind(cursor_at)
            .fetch_all(pool)
            .await
        })?;
        if !self.primed {
            self.primed = true;
            for (id, at) in rows {
                self.sent(id, at);
            }
            return Ok(Vec::new());
        }
        let mut missed = Vec::new();
        for (id, at) in rows {
            if self.sent.contains(&id) {
                continue;
            }
            self.sent(id, at);
            if let Some(message) = state.message_by_id(id, Some(viewer_id)).await? {
                missed.push(message);
            }
        }
        Ok(missed)
    }
}
