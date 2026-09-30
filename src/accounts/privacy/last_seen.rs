//! Persisted last-seen and its obscured tiers.
//!
//! The leak this file exists to close: if an obscured tier ("recently", "within a week", …)
//! were computed from the exact last-seen time, a viewer polling it would see the tier flip
//! exactly N days after the owner's last activity — and recover the exact time by
//! subtraction. Worse, a flip back to "recently" at the moment the owner comes online
//! would reveal the online transition itself.
//!
//! So the obscured tier is a function of two UTC calendar days only: today, and the day of
//! the owner's latest activity *strictly before today*. Activity during today is invisible
//! to obscured viewers until midnight. Consequently every obscured tier is constant between
//! two UTC midnights, whatever happens in between and however often it is requested: the
//! most a viewer can learn is "active on some day in this range", never a time of day.
//! Telegram's exact thresholds are unpublished; the ones here (3 / 7 / 30 days) follow its
//! documented wording.

use chrono::{DateTime, NaiveDate, Utc};
use uuid::Uuid;

use crate::models::UserStatus;
use crate::state::{with_pool, AppState};

/// Activity within this many whole days before today reads as "recently".
const RECENTLY_DAYS: i64 = 3;
const WITHIN_WEEK_DAYS: i64 = 7;
const WITHIN_MONTH_DAYS: i64 = 30;

/// One account's persisted last-seen state (`user_last_seen`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LastSeenRecord {
    /// Exact time of the latest observed activity.
    pub last_seen_at: DateTime<Utc>,
    /// UTC day of the latest activity strictly before `last_seen_at`'s day.
    pub prior_seen_day: Option<NaiveDate>,
}

impl LastSeenRecord {
    /// The record after observing activity at `at`. Out-of-order observations (an older
    /// `at`) change nothing, so concurrent sockets cannot move last-seen backwards.
    pub fn touched(previous: Option<Self>, at: DateTime<Utc>) -> Self {
        let Some(previous) = previous else {
            return Self {
                last_seen_at: at,
                prior_seen_day: None,
            };
        };
        if at <= previous.last_seen_at {
            return previous;
        }
        let previous_day = previous.last_seen_at.date_naive();
        Self {
            last_seen_at: at,
            prior_seen_day: if at.date_naive() > previous_day {
                Some(previous_day)
            } else {
                previous.prior_seen_day
            },
        }
    }

    /// The UTC day of the latest activity before `today` — the only input an obscured
    /// viewer's answer may depend on.
    fn activity_day_before(&self, today: NaiveDate) -> Option<NaiveDate> {
        let day = self.last_seen_at.date_naive();
        if day < today {
            Some(day)
        } else {
            self.prior_seen_day.filter(|prior| *prior < today)
        }
    }
}

/// The status a viewer who may not see `record`'s owner's exact last-seen receives.
/// Never `Online`, never a timestamp. An account with no activity before today reads as
/// "long ago" — including a brand-new one, which is the price of never reacting to today.
pub fn obscured_status(record: Option<&LastSeenRecord>, now: DateTime<Utc>) -> UserStatus {
    let today = now.date_naive();
    let Some(day) = record.and_then(|record| record.activity_day_before(today)) else {
        return UserStatus::LongAgo;
    };
    let age_days = (today - day).num_days();
    if age_days <= RECENTLY_DAYS {
        UserStatus::Recently
    } else if age_days <= WITHIN_WEEK_DAYS {
        UserStatus::WithinWeek
    } else if age_days <= WITHIN_MONTH_DAYS {
        UserStatus::WithinMonth
    } else {
        UserStatus::LongAgo
    }
}

/// The status a viewer admitted by the owner's `last_seen` rule receives.
pub(crate) fn exact_status(record: Option<&LastSeenRecord>, online: bool) -> UserStatus {
    if online {
        return UserStatus::Online;
    }
    match record {
        Some(record) => UserStatus::Offline {
            last_seen: record.last_seen_at,
        },
        None => UserStatus::Empty,
    }
}

type LastSeenRow = (DateTime<Utc>, Option<NaiveDate>);

impl AppState {
    pub async fn last_seen_record(
        &self,
        user_id: Uuid,
    ) -> Result<Option<LastSeenRecord>, sqlx::Error> {
        let row: Option<LastSeenRow> = with_pool!(self, |pool| {
            sqlx::query_as(
                "SELECT last_seen_at, prior_seen_day FROM user_last_seen WHERE user_id = $1",
            )
            .bind(user_id)
            .fetch_optional(pool)
            .await
        })?;
        Ok(row.map(|(last_seen_at, prior_seen_day)| LastSeenRecord {
            last_seen_at,
            prior_seen_day,
        }))
    }

    /// Record activity at `at` (connect, heartbeat, disconnect). Read-modify-write: two
    /// racing touches of one account both derive from the same previous row and write
    /// records that differ only in which of two near-identical instants wins.
    pub(crate) async fn touch_last_seen(
        &self,
        user_id: Uuid,
        at: DateTime<Utc>,
    ) -> Result<LastSeenRecord, sqlx::Error> {
        let previous = self.last_seen_record(user_id).await?;
        let next = LastSeenRecord::touched(previous, at);
        if Some(next) == previous {
            return Ok(next);
        }
        with_pool!(self, |pool| {
            sqlx::query(
                "INSERT INTO user_last_seen (user_id, last_seen_at, prior_seen_day) \
                 SELECT users.id, $2, $3 FROM users WHERE users.id = $1 \
                 ON CONFLICT (user_id) DO UPDATE SET \
                 last_seen_at = excluded.last_seen_at, prior_seen_day = excluded.prior_seen_day",
            )
            .bind(user_id)
            .bind(next.last_seen_at)
            .bind(next.prior_seen_day)
            .execute(pool)
            .await
            .map(|_| ())
        })?;
        Ok(next)
    }

    /// Records of every active member of one chat, for the `auth_ok` snapshot.
    pub(crate) async fn chat_last_seen_records(
        &self,
        room_id: Uuid,
    ) -> Result<Vec<(Uuid, LastSeenRecord)>, sqlx::Error> {
        let rows: Vec<(Uuid, DateTime<Utc>, Option<NaiveDate>)> = with_pool!(self, |pool| {
            sqlx::query_as(
                "SELECT seen.user_id, seen.last_seen_at, seen.prior_seen_day \
                 FROM user_last_seen AS seen JOIN chat_members AS members \
                   ON members.user_id = seen.user_id AND members.room_id = $1 \
                   AND members.status = 'active'",
            )
            .bind(room_id)
            .fetch_all(pool)
            .await
        })?;
        Ok(rows
            .into_iter()
            .map(|(user_id, last_seen_at, prior_seen_day)| {
                (
                    user_id,
                    LastSeenRecord {
                        last_seen_at,
                        prior_seen_day,
                    },
                )
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn at(day: u32, hour: u32, minute: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 10, day, hour, minute, 0)
            .unwrap()
    }

    #[test]
    fn crossing_a_day_remembers_the_previous_activity_day() {
        let first = LastSeenRecord::touched(None, at(1, 9, 0));
        let same_day = LastSeenRecord::touched(Some(first), at(1, 22, 0));
        assert_eq!(same_day.prior_seen_day, None);
        let next_day = LastSeenRecord::touched(Some(same_day), at(4, 8, 0));
        assert_eq!(next_day.prior_seen_day, Some(at(1, 0, 0).date_naive()));
        assert_eq!(
            LastSeenRecord::touched(Some(next_day), at(2, 0, 0)),
            next_day
        );
    }

    #[test]
    fn today_is_invisible_to_obscured_viewers() {
        let old = LastSeenRecord::touched(None, at(1, 9, 0));
        let back_today = LastSeenRecord::touched(Some(old), at(20, 14, 0));
        // Online right now, but the obscured answer still reflects October 1st.
        assert_eq!(
            obscured_status(Some(&back_today), at(20, 14, 1)),
            UserStatus::WithinMonth
        );
        assert_eq!(
            obscured_status(Some(&back_today), at(21, 0, 0)),
            UserStatus::Recently
        );
    }
}
