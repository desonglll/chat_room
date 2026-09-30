//! TG-406 acceptance: concurrent ballots stay consistent on SQLite *and* PostgreSQL.
//!
//! Every scenario fires many overlapping transactions at one poll through the module
//! interface and then checks the database directly: one ballot per voter in a single-choice
//! poll, never a mixture of two ballots in a multiple-choice poll, exactly one accepted answer
//! per quiz voter, and aggregate counts equal to the rows they summarise.

mod migration_support;
mod poll_support;

use std::collections::HashMap;
use std::sync::Arc;

use chat_room::config::AppConfig;
use chat_room::messages::polls::{cast_vote, model::PollError, poll_for_viewer, retract_vote};
use chat_room::state::AppState;
use migration_support::{create_postgres_scratch, drop_postgres_scratch, postgres_admin_pool};
use poll_support::*;
use serde_json::json;
use uuid::Uuid;

const VOTERS: usize = 40;
const ATTEMPTS: usize = 5;

type Rows = Vec<(Uuid, i32)>;

/// Every `(user_id, position)` of one poll, read straight from `poll_votes`.
macro_rules! vote_rows {
    ($pool:expr, $poll:expr) => {{
        let rows: Rows =
            sqlx::query_as("SELECT user_id, position FROM poll_votes WHERE message_id = $1")
                .bind($poll)
                .fetch_all($pool)
                .await
                .unwrap();
        let mut by_user: HashMap<Uuid, Vec<i32>> = HashMap::new();
        for (user, position) in rows {
            by_user.entry(user).or_default().push(position);
        }
        for positions in by_user.values_mut() {
            positions.sort_unstable();
        }
        by_user
    }};
}

async fn fire<F, Fut>(voters: &[Uuid], attempts: usize, ballot: F) -> Vec<Result<(), PollError>>
where
    F: Fn(usize, usize, Uuid) -> Fut,
    Fut: std::future::Future<Output = Result<(), PollError>> + Send + 'static,
{
    let tasks = voters
        .iter()
        .enumerate()
        .flat_map(|(index, voter)| (0..attempts).map(move |attempt| (index, attempt, *voter)));
    let handles: Vec<_> = tasks
        .map(|(index, attempt, voter)| tokio::spawn(ballot(index, attempt, voter)))
        .collect();
    futures_util::future::join_all(handles)
        .await
        .into_iter()
        .map(|joined| joined.unwrap())
        .collect()
}

/// The three scenarios, run against whichever adapter `server` was built on. `$pool` reads
/// the rows back so the assertion does not trust the code under test.
macro_rules! consistency_scenarios {
    ($server:expr, $pool:expr, $label:expr) => {{
        let server = &$server;
        let base = &server.base;
        let alice = register(base, &format!("{}-alice", $label)).await;
        let chat = create_chat(base, &alice, $label).await;
        let voters = seed_members!($pool, chat, VOTERS);
        let options = json!(["a", "b", "c", "d", "e"]);

        // 1. Single choice: each voter races five different ballots. One survives per voter.
        let single = poll(base, &alice, chat, json!({ "question": "single", "options": options })).await;
        let results = fire(&voters, ATTEMPTS, |_, attempt, voter| {
            let state = server.state.clone();
            async move { cast_vote(&state, single, voter, &[attempt as u32]).await.map(|_| ()) }
        })
        .await;
        assert!(results.iter().all(Result::is_ok), "{:?}", results.iter().find(|r| r.is_err()));
        let rows = vote_rows!($pool, single);
        assert_eq!(rows.len(), VOTERS);
        assert!(rows.values().all(|positions| positions.len() == 1));
        let state = poll_for_viewer(&server.state, single, alice.id).await.unwrap();
        assert_eq!(state.total_voters, VOTERS as i64);
        assert_eq!(state.options.iter().map(|o| o.voters).sum::<i64>(), VOTERS as i64);

        // 2. Multiple choice: {0,1} races {2,3,4} and a retraction. The survivor is exactly
        //    one of them — never a union of two ballots.
        let multi = poll(
            base,
            &alice,
            chat,
            json!({ "question": "multi", "options": options, "multiple_choice": true }),
        )
        .await;
        let results = fire(&voters, 3, |_, attempt, voter| {
            let state = server.state.clone();
            async move {
                match attempt {
                    0 => cast_vote(&state, multi, voter, &[0, 1]).await.map(|_| ()),
                    1 => cast_vote(&state, multi, voter, &[2, 3, 4]).await.map(|_| ()),
                    _ => retract_vote(&state, multi, voter).await.map(|_| ()),
                }
            }
        })
        .await;
        assert!(results.iter().all(Result::is_ok));
        let rows = vote_rows!($pool, multi);
        for positions in rows.values() {
            assert!(
                positions == &vec![0, 1] || positions == &vec![2, 3, 4],
                "mixed ballot {positions:?}"
            );
        }
        let state = poll_for_viewer(&server.state, multi, alice.id).await.unwrap();
        assert_eq!(state.total_voters, rows.len() as i64);
        let counted: i64 = state.options.iter().map(|o| o.voters).sum();
        assert_eq!(counted, rows.values().map(|p| p.len() as i64).sum::<i64>());

        // 3. Quiz: five racing answers per voter, exactly one accepted.
        let quiz = poll(
            base,
            &alice,
            chat,
            json!({ "question": "quiz", "options": options, "quiz": true, "correct_option": 2 }),
        )
        .await;
        let results = fire(&voters, ATTEMPTS, |_, attempt, voter| {
            let state = server.state.clone();
            async move { cast_vote(&state, quiz, voter, &[attempt as u32]).await.map(|_| ()) }
        })
        .await;
        let accepted = results.iter().filter(|r| r.is_ok()).count();
        let refused = results
            .iter()
            .filter(|r| matches!(r, Err(PollError::QuizAnswered)))
            .count();
        assert_eq!((accepted, refused), (VOTERS, VOTERS * (ATTEMPTS - 1)));
        let rows = vote_rows!($pool, quiz);
        assert_eq!(rows.len(), VOTERS);
        assert!(rows.values().all(|positions| positions.len() == 1));
    }};
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_votes_are_consistent_on_sqlite() {
    let database = migration_support::sqlite_scratch_path("poll-consistency");
    let server = serve(Arc::new(AppState::open(&database).await.unwrap())).await;
    consistency_scenarios!(server, server.state.pool(), "sqlite-consistency");
    drop(server);
    migration_support::remove_sqlite_files(&database);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_votes_are_consistent_on_postgres() {
    let Some((admin_url, admin_pool)) =
        postgres_admin_pool("concurrent_votes_are_consistent_on_postgres").await
    else {
        return;
    };
    let scratch = create_postgres_scratch(&admin_url, &admin_pool, "poll_consistency").await;
    let state = AppState::open_postgres(&scratch.url, &AppConfig::default())
        .await
        .unwrap();
    let server = serve(Arc::new(state)).await;
    let pool = server.state.postgres_pool().unwrap().clone();
    consistency_scenarios!(server, &pool, "pg-consistency");
    pool.close().await;
    drop(server);
    drop_postgres_scratch(&admin_pool, &scratch).await;
}
