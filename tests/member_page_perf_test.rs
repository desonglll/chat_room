//! TG-201 acceptance: paging a 200 000-member supergroup roster. SQLite always (file-backed,
//! like production), PostgreSQL when configured.
//!
//! Asserts correctness (every member exactly once, in `(joined_at, user_id) DESC` order, the
//! member_count triggers agreeing with the seeded rows) and a generous latency ceiling that
//! only a full scan would break; the measured timings are printed for the devlog:
//!
//! ```sh
//! cargo test --test member_page_perf_test -- --nocapture 2>&1 | grep 'tg201-perf'
//! ```

use std::sync::Arc;
use std::time::{Duration, Instant};

use chat_room::chats::member_page::{MemberCursor, MemberRow};
use chat_room::chats::ChatType;
use chat_room::state::AppState;
use uuid::Uuid;

mod chat_admin_support;
mod roster_seed;

use chat_admin_support::{serve, with_postgres};
use roster_seed::seed_members;

const MEMBERS: i64 = 200_000;
const PAGE: i64 = 100;
/// A keyset page is a bounded index range scan; even on a loaded CI box it stays far below
/// this. A regression to a scan of the whole roster would not.
const PAGE_CEILING: Duration = Duration::from_millis(250);

fn cursor_of(row: &MemberRow) -> MemberCursor {
    MemberCursor {
        joined_at: row.joined_at.unwrap(),
        user_id: row.user_id,
    }
}

async fn timed_page(
    state: &AppState,
    room_id: Uuid,
    after: Option<MemberCursor>,
) -> (Vec<MemberRow>, Duration) {
    let started = Instant::now();
    let rows = state.member_page_rows(room_id, after, PAGE).await.unwrap();
    (rows, started.elapsed())
}

async fn roster_scenario(label: &str, state: Arc<AppState>) {
    let server = serve(state.clone()).await;
    let owner = server.register(&format!("tg201-perf-owner-{label}")).await;
    let chat = server
        .create_group(&owner, &format!("tg201-perf-{label}"))
        .await;
    let room_id = Uuid::parse_str(&chat).unwrap();

    let started = Instant::now();
    seed_members(&state, room_id, &format!("tg201-perf-{label}-"), MEMBERS).await;
    let seeding = started.elapsed();
    // The statement-level (PostgreSQL) / row-level (SQLite) triggers counted every row.
    assert_eq!(server.member_count(&chat, &owner.token).await, MEMBERS + 1);
    // A bulk insert bypasses the domain upgrade; the next membership write applies it.
    state.settle_for_test(room_id).await;
    assert_eq!(
        state.chat_type(room_id).await.unwrap(),
        Some(ChatType::Supergroup)
    );

    // First page, a page in the middle, and the last page cost the same.
    let (first, first_time) = timed_page(&state, room_id, None).await;
    assert_eq!(first.len() as i64, PAGE);
    let mut cursor = cursor_of(first.last().unwrap());
    let mut seen = std::collections::HashSet::new();
    seen.extend(first.iter().map(|row| row.user_id));
    let mut previous = cursor;
    let mut slowest = first_time;
    let mut total = first_time;
    let mut pages = 1;
    let walk = Instant::now();
    let mut middle_time = Duration::ZERO;
    let mut last_time = Duration::ZERO;
    loop {
        let (rows, elapsed) = timed_page(&state, room_id, Some(cursor)).await;
        if rows.is_empty() {
            break;
        }
        pages += 1;
        total += elapsed;
        slowest = slowest.max(elapsed);
        if pages == (MEMBERS / PAGE / 2) as usize {
            middle_time = elapsed;
        }
        last_time = elapsed;
        for row in &rows {
            let key = cursor_of(row);
            assert!(
                (key.joined_at, key.user_id) < (previous.joined_at, previous.user_id),
                "strictly descending keyset order"
            );
            previous = key;
            assert!(seen.insert(row.user_id), "a member appeared twice");
        }
        cursor = cursor_of(rows.last().unwrap());
    }
    let walk_time = walk.elapsed();
    assert_eq!(seen.len() as i64, MEMBERS + 1, "every member exactly once");

    // For the devlog: what OFFSET would cost at the same depth.
    let offset_time = state
        .offset_probe_for_test(room_id, MEMBERS - PAGE, PAGE)
        .await;

    eprintln!(
        "tg201-perf {label}: seed {MEMBERS} members {seeding:?}; page size {PAGE}; \
         first {first_time:?}; middle {middle_time:?}; last {last_time:?}; \
         slowest {slowest:?}; mean {:?} over {pages} pages; full walk {walk_time:?}; \
         OFFSET {} page {offset_time:?}",
        total / pages as u32,
        MEMBERS - PAGE,
    );
    assert!(
        first_time < PAGE_CEILING && middle_time < PAGE_CEILING && last_time < PAGE_CEILING,
        "keyset pages must not degrade with depth: first {first_time:?}, middle \
         {middle_time:?}, last {last_time:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn member_page_200k_on_sqlite() {
    let path = std::env::temp_dir().join(format!("tg201-perf-{}.db", Uuid::new_v4()));
    let state = Arc::new(AppState::open(&path).await.unwrap());
    roster_scenario("sqlite", state.clone()).await;
    state.pool().close().await;
    drop(state);
    for suffix in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{suffix}", path.display()));
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn member_page_200k_on_postgres() {
    with_postgres("member_page_200k_on_postgres", |state| {
        roster_scenario("postgres", state)
    })
    .await;
}

/// Test-only probes, expressed through the public pool accessors so the production
/// interface is not widened for them.
trait PerfProbes {
    async fn settle_for_test(&self, room_id: Uuid);
    async fn offset_probe_for_test(&self, room_id: Uuid, offset: i64, limit: i64) -> Duration;
}

impl PerfProbes for AppState {
    async fn settle_for_test(&self, room_id: Uuid) {
        // Any domain membership write runs the upgrade; re-joining an active member is the
        // cheapest one and leaves the count unchanged.
        let anyone = self
            .member_page_rows(room_id, None, 1)
            .await
            .unwrap()
            .pop()
            .unwrap();
        self.request_chat_membership(room_id, anyone.user_id, true)
            .await
            .unwrap();
    }

    async fn offset_probe_for_test(&self, room_id: Uuid, offset: i64, limit: i64) -> Duration {
        let query = "SELECT user_id FROM chat_members WHERE room_id = $1 AND status = 'active' \
                     ORDER BY joined_at DESC, user_id DESC LIMIT $2 OFFSET $3";
        let started = Instant::now();
        if let Some(pool) = self.postgres_pool() {
            let rows: Vec<(Uuid,)> = sqlx::query_as(query)
                .bind(room_id)
                .bind(limit)
                .bind(offset)
                .fetch_all(pool)
                .await
                .unwrap();
            assert_eq!(rows.len() as i64, limit);
        } else {
            let rows: Vec<(Uuid,)> = sqlx::query_as(query)
                .bind(room_id)
                .bind(limit)
                .bind(offset)
                .fetch_all(self.pool())
                .await
                .unwrap();
            assert_eq!(rows.len() as i64, limit);
        }
        started.elapsed()
    }
}
