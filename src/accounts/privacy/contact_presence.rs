//! TG-903: the viewer's friends' presence for the contacts list (online / last seen), under
//! exactly the same `last_seen` rules as the WebSocket snapshot in `presence.rs`: exact status
//! only when both sides' rules admit each other, otherwise the obscured tier.

use std::collections::HashSet;

use chrono::{DateTime, NaiveDate, Utc};
use uuid::Uuid;

use super::last_seen::{exact_status, obscured_status, LastSeenRecord};
use super::presence::LAST_SEEN;
use super::PrivacyTier;
use crate::models::UserStatusEntry;
use crate::state::{with_pool, AppState};

/// A friend's id, `last_seen` rule tier, and last-seen record columns.
type FriendPresenceRow = (
    Uuid,
    Option<String>,
    Option<DateTime<Utc>>,
    Option<NaiveDate>,
);

impl AppState {
    /// One entry per accepted friend of `viewer`.
    pub(crate) async fn friend_statuses(
        &self,
        viewer: Uuid,
        now: DateTime<Utc>,
    ) -> Result<Vec<UserStatusEntry>, sqlx::Error> {
        // The room-scoped context with no room reads no tiers; fill them for the friends.
        let mut context = self.viewer_context(viewer, Uuid::nil()).await?;
        let rows: Vec<FriendPresenceRow> = with_pool!(self, |pool| {
            sqlx::query_as(
                "SELECT friends.id, rules.tier, seen.last_seen_at, seen.prior_seen_day \
                     FROM (SELECT CASE WHEN user_low_id = $1 THEN user_high_id \
                             ELSE user_low_id END AS id \
                           FROM friendships WHERE status = 'accepted' \
                             AND (user_low_id = $1 OR user_high_id = $1)) AS friends \
                     LEFT JOIN user_privacy_rules AS rules ON rules.user_id = friends.id \
                       AND rules.privacy_key = $2 \
                     LEFT JOIN user_last_seen AS seen ON seen.user_id = friends.id",
            )
            .bind(viewer)
            .bind(LAST_SEEN)
            .fetch_all(pool)
            .await
        })?;
        for (id, tier, _, _) in &rows {
            if let Some(tier) = tier {
                context.owner_tiers.insert(*id, PrivacyTier::parse(tier));
            }
        }
        let online: HashSet<Uuid> = {
            let chats = self.members.read().await;
            chats
                .values()
                .flat_map(|members| members.keys().copied())
                .collect()
        };
        Ok(rows
            .into_iter()
            .map(|(id, _, last_seen_at, prior_seen_day)| {
                let record = last_seen_at.map(|last_seen_at| LastSeenRecord {
                    last_seen_at,
                    prior_seen_day,
                });
                UserStatusEntry {
                    user_id: id,
                    status: if context.exact_visible(id) {
                        exact_status(record.as_ref(), online.contains(&id))
                    } else {
                        obscured_status(record.as_ref(), now)
                    },
                }
            })
            .collect())
    }
}
