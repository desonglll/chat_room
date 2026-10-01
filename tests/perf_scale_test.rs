//! TG-604 scale scenarios not already covered elsewhere, on file-backed SQLite (like
//! production): a chat with 100 000 messages (newest page, a page deep in history, a jump to
//! the oldest message) and a conversation list of 500 chats. Generous ceilings that only a scan
//! or an N+1 regression would break; the measured timings are printed for the results table:
//!
//! ```sh
//! cargo test --test perf_scale_test -- --nocapture 2>&1 | grep tg604-perf
//! ```

use std::sync::Arc;
use std::time::{Duration, Instant};

use chat_room::state::AppState;
use reqwest::Method;
use serde_json::Value;
use uuid::Uuid;

mod chat_admin_support;
mod roster_seed;

use chat_admin_support::serve;
use roster_seed::SEED_EPOCH;

const MESSAGES: i64 = 100_000;
const CHATS: usize = 500;
const PAGE_CEILING: Duration = Duration::from_millis(300);
const LIST_CEILING: Duration = Duration::from_millis(2_000);

async fn temp_state(label: &str) -> (Arc<AppState>, std::path::PathBuf) {
    let path = std::env::temp_dir().join(format!("tg604-{label}-{}.db", Uuid::new_v4()));
    (Arc::new(AppState::open(&path).await.unwrap()), path)
}

async fn cleanup(state: Arc<AppState>, path: std::path::PathBuf) {
    state.pool().close().await;
    drop(state);
    for suffix in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{suffix}", path.display()));
    }
}

/// Median of `runs` timings of `request` (the first, cold run included in the max only).
async fn timed<F, Fut>(runs: usize, mut request: F) -> (Duration, Duration, Value)
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Value>,
{
    let mut samples = Vec::with_capacity(runs);
    let mut last = Value::Null;
    for _ in 0..runs {
        let started = Instant::now();
        last = request().await;
        samples.push(started.elapsed());
    }
    let max = *samples.iter().max().unwrap();
    samples.sort();
    (samples[runs / 2], max, last)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_chat_with_100k_messages_pages_and_jumps_in_bounded_time() {
    let (state, path) = temp_state("history").await;
    let server = serve(state.clone()).await;
    let owner = server.register("tg604-history-owner").await;
    let chat = server.create_group(&owner, "tg604-history").await;
    let owner_id: Uuid = owner.id.parse().unwrap();

    let started = Instant::now();
    sqlx::query(
        "WITH RECURSIVE seq(n) AS (SELECT 1 UNION ALL SELECT n + 1 FROM seq WHERE n < $1) \
         INSERT INTO messages (id, room_id, sender_id, sender, content, created_at) \
         SELECT randomblob(16), $2, $3, 'tg604', 'message number ' || n, \
           strftime('%Y-%m-%dT%H:%M:%S+00:00', $4 + n, 'unixepoch') FROM seq",
    )
    .bind(MESSAGES)
    .bind(Uuid::parse_str(&chat).unwrap())
    .bind(owner_id)
    .bind(SEED_EPOCH)
    .execute(state.pool())
    .await
    .unwrap();
    let seeding = started.elapsed();

    let path_of = |suffix: &str| format!("/api/chats/{chat}{suffix}");
    let (newest, newest_max, page) = timed(5, || async {
        server
            .get(&path_of("/messages?limit=50"), &owner.token)
            .await
            .1
    })
    .await;
    assert_eq!(page.as_array().unwrap().len(), 50);

    // 1 000 pages back: the cursor is the 50 000th message.
    let (deep_cursor,): (Uuid,) = sqlx::query_as(
        "SELECT id FROM messages WHERE content = 'message number 50000' AND room_id = $1",
    )
    .bind(Uuid::parse_str(&chat).unwrap())
    .fetch_one(state.pool())
    .await
    .unwrap();
    let (deep, deep_max, page) = timed(5, || async {
        server
            .get(
                &path_of(&format!("/messages?limit=50&before={deep_cursor}")),
                &owner.token,
            )
            .await
            .1
    })
    .await;
    assert_eq!(page.as_array().unwrap().len(), 50);

    let (oldest_id,): (Uuid,) =
        sqlx::query_as("SELECT id FROM messages WHERE room_id = $1 ORDER BY created_at LIMIT 1")
            .bind(Uuid::parse_str(&chat).unwrap())
            .fetch_one(state.pool())
            .await
            .unwrap();
    let (jump, jump_max, context) = timed(5, || async {
        server
            .call(
                Method::GET,
                &path_of(&format!("/messages/{oldest_id}/context")),
                &owner.token,
                None,
            )
            .await
            .1
    })
    .await;
    assert!(
        context.to_string().contains("message number 1\""),
        "the oldest message is in its window"
    );

    eprintln!(
        "tg604-perf history: seed {MESSAGES} messages {seeding:?}; newest page median {newest:?} \
         (max {newest_max:?}); page 1000 back median {deep:?} (max {deep_max:?}); jump to the \
         oldest median {jump:?} (max {jump_max:?})"
    );
    for (name, value) in [("newest", newest), ("deep", deep), ("jump", jump)] {
        assert!(value < PAGE_CEILING, "{name} page took {value:?}");
    }
    cleanup(state, path).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_list_of_500_conversations_loads_in_bounded_time() {
    let (state, path) = temp_state("list").await;
    let server = serve(state.clone()).await;
    let owner = server.register("tg604-list-owner").await;
    let owner_id: Uuid = owner.id.parse().unwrap();
    let started = Instant::now();
    for index in 0..CHATS {
        let chat = server
            .create_group(&owner, &format!("tg604-list-{index}"))
            .await;
        sqlx::query(
            "INSERT INTO messages (id, room_id, sender_id, sender, content, created_at) \
             VALUES (randomblob(16), $1, $2, 'tg604', 'last words', $3)",
        )
        .bind(Uuid::parse_str(&chat).unwrap())
        .bind(owner_id)
        .bind(chrono::Utc::now())
        .execute(state.pool())
        .await
        .unwrap();
    }
    let seeding = started.elapsed();

    let (median, max, list) = timed(5, || async {
        server.get("/api/conversations", &owner.token).await.1
    })
    .await;
    assert_eq!(list.as_array().unwrap().len(), CHATS);
    let (chats_median, chats_max, chats) = timed(5, || async {
        server.get("/api/chats", &owner.token).await.1
    })
    .await;
    assert_eq!(chats.as_array().unwrap().len(), CHATS);
    eprintln!(
        "tg604-perf list: seed {CHATS} chats {seeding:?}; /api/conversations median {median:?} \
         (max {max:?}); /api/chats median {chats_median:?} (max {chats_max:?})"
    );
    assert!(median < LIST_CEILING, "conversation list took {median:?}");
    assert!(
        chats_median < LIST_CEILING,
        "chat list took {chats_median:?}"
    );
    cleanup(state, path).await;
}
