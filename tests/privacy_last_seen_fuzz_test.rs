//! TG-505's dedicated leak test: the obscured last-seen tiers must not let a viewer who
//! polls them reconstruct the exact last-seen time.
//!
//! Attack model: a viewer the owner hides from requests the owner's status as often as
//! they like, at any instants they like, and watches for the moment a tier changes. If the
//! tier were computed from the exact time, the change would land exactly N days after the
//! owner's activity (or exactly when the owner comes online), and subtraction would give
//! the exact time back. The tests below prove the answer depends only on UTC calendar days.

mod privacy_support;

use chat_room::accounts::privacy::{obscured_status, LastSeenRecord};
use chat_room::models::UserStatus;
use chrono::{DateTime, Duration, NaiveDate, TimeZone, Utc};
use privacy_support::{start_server, status_of};

fn instant(day: NaiveDate, seconds: i64) -> DateTime<Utc> {
    Utc.from_utc_datetime(&day.and_hms_opt(0, 0, 0).unwrap()) + Duration::seconds(seconds)
}

/// Build the record an account ends up with after activity at each of `activity`.
fn record_after(activity: &[DateTime<Utc>]) -> LastSeenRecord {
    activity
        .iter()
        .fold(None, |record, at| {
            Some(LastSeenRecord::touched(record, *at))
        })
        .unwrap()
}

/// Every probe instant over 45 days at a 7-minute stride (not a divisor of a day, so the
/// probes drift across every time of day).
fn probes(from: DateTime<Utc>) -> impl Iterator<Item = DateTime<Utc>> {
    (0..(45 * 24 * 60 / 7)).map(move |step| from + Duration::minutes(7 * step))
}

#[test]
fn two_owners_last_seen_at_opposite_ends_of_a_day_are_indistinguishable_forever() {
    let day = NaiveDate::from_ymd_opt(2026, 10, 1).unwrap();
    let earliest = record_after(&[instant(day, 1)]);
    let latest = record_after(&[instant(day, 86_399)]);
    for now in probes(instant(day, 0)) {
        assert_eq!(
            obscured_status(Some(&earliest), now),
            obscured_status(Some(&latest), now),
            "a probe at {now} distinguishes 00:00:01 from 23:59:59"
        );
    }
}

#[test]
fn obscured_tiers_only_ever_change_at_utc_midnight() {
    let day = NaiveDate::from_ymd_opt(2026, 10, 1).unwrap();
    // A realistic history: activity on several days at arbitrary times.
    let record = record_after(&[
        instant(day, 3_600 * 9 + 17),
        instant(day, 3_600 * 22 + 5),
        instant(day + Duration::days(2), 3_600 * 13 + 41),
    ]);
    for now in probes(instant(day, 0)) {
        let midnight = instant(now.date_naive(), 0);
        assert_eq!(
            obscured_status(Some(&record), now),
            obscured_status(Some(&record), midnight),
            "the tier at {now} differs from the tier at that day's midnight"
        );
    }
}

#[test]
fn coming_online_does_not_change_the_answer_before_midnight() {
    let day = NaiveDate::from_ymd_opt(2026, 10, 1).unwrap();
    let away = record_after(&[instant(day, 3_600 * 8)]);
    let comeback = instant(day + Duration::days(10), 3_600 * 14 + 23 * 60);
    let back = LastSeenRecord::touched(Some(away), comeback);
    // From the instant the owner reconnects until midnight, the answer a hidden viewer
    // gets is the one they got before the owner came back.
    let mut now = comeback;
    while now.date_naive() == comeback.date_naive() {
        assert_eq!(
            obscured_status(Some(&back), now),
            obscured_status(Some(&away), now)
        );
        now += Duration::minutes(1);
    }
    assert_eq!(obscured_status(Some(&back), now), UserStatus::Recently);
}

#[test]
fn the_tiers_follow_whole_days_since_the_last_active_day() {
    let day = NaiveDate::from_ymd_opt(2026, 10, 1).unwrap();
    let record = record_after(&[instant(day, 3_600 * 12)]);
    let at_day = |offset: i64| instant(day + Duration::days(offset), 3_600 * 6);
    assert_eq!(
        obscured_status(Some(&record), at_day(1)),
        UserStatus::Recently
    );
    assert_eq!(
        obscured_status(Some(&record), at_day(3)),
        UserStatus::Recently
    );
    assert_eq!(
        obscured_status(Some(&record), at_day(4)),
        UserStatus::WithinWeek
    );
    assert_eq!(
        obscured_status(Some(&record), at_day(7)),
        UserStatus::WithinWeek
    );
    assert_eq!(
        obscured_status(Some(&record), at_day(8)),
        UserStatus::WithinMonth
    );
    assert_eq!(
        obscured_status(Some(&record), at_day(30)),
        UserStatus::WithinMonth
    );
    assert_eq!(
        obscured_status(Some(&record), at_day(31)),
        UserStatus::LongAgo
    );
    assert_eq!(obscured_status(None, at_day(0)), UserStatus::LongAgo);
    for offset in 0..40 {
        assert!(!matches!(
            obscured_status(Some(&record), at_day(offset)),
            UserStatus::Online | UserStatus::Offline { .. }
        ));
    }
}

/// End to end: two owners whose persisted last-seen differs by almost a whole day are
/// reported identically to a hidden viewer on every one of many requests, while an
/// admitted viewer (the control) does see the difference.
#[tokio::test]
async fn repeated_requests_through_the_server_reveal_nothing_finer_than_a_day() {
    let server = start_server().await;
    let early = server.account("fz-early").await;
    let late = server.account("fz-late").await;
    let hidden = server.account("fz-hidden").await;
    let admitted = server.account("fz-admitted").await;
    let group = server.create_group(&early, "fz-group").await;
    for account in [&early, &late, &hidden, &admitted] {
        server.join(group, account).await;
    }
    for owner in [&early, &late] {
        server
            .put_rule(owner, "last_seen", "nobody", &[&admitted], &[])
            .await;
    }
    let yesterday = Utc::now().date_naive() - Duration::days(1);
    for (owner, at) in [
        (&early, instant(yesterday, 60)),
        (&late, instant(yesterday, 86_340)),
    ] {
        let pool = server.state.pool();
        sqlx::query(
            "UPDATE user_last_seen SET last_seen_at = $1, prior_seen_day = NULL \
             WHERE user_id = $2",
        )
        .bind(at)
        .bind(owner.id)
        .execute(pool)
        .await
        .unwrap();
    }

    for _ in 0..20 {
        let (socket, auth) = server.open_chat(group, &hidden).await;
        let (early_status, late_status) = (status_of(&auth, early.id), status_of(&auth, late.id));
        assert_eq!(early_status, serde_json::json!({ "kind": "recently" }));
        assert_eq!(early_status, late_status);
        server.close_and_settle(group, &hidden, socket).await;
    }

    let (socket, auth) = server.open_chat(group, &admitted).await;
    let early_exact = status_of(&auth, early.id)["last_seen"].clone();
    let late_exact = status_of(&auth, late.id)["last_seen"].clone();
    assert!(early_exact.is_string() && late_exact.is_string());
    assert_ne!(early_exact, late_exact);
    server.close_and_settle(group, &admitted, socket).await;
}
