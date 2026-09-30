//! TG-406 acceptance: 1000 concurrent voters produce a bounded number of `poll_updated`
//! broadcasts, and the last one carries the exact final tally.

mod poll_support;

use std::sync::Arc;
use std::time::{Duration, Instant};

use chat_room::messages::polls::{cast_vote, POLL_BROADCAST_WINDOW};
use chat_room::state::AppState;
use poll_support::*;
use serde_json::{json, Value};

const VOTERS: usize = 1000;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_thousand_concurrent_voters_cost_a_bounded_number_of_broadcasts() {
    let database =
        std::env::temp_dir().join(format!("chat-room-poll-load-{}.db", uuid::Uuid::new_v4()));
    let server = serve(Arc::new(AppState::open(&database).await.unwrap())).await;
    let base = &server.base;
    let alice = register(base, "load-alice").await;
    let chat = create_chat(base, &alice, "load").await;
    let poll = poll(
        base,
        &alice,
        chat,
        json!({ "question": "Load?", "options": ["a", "b", "c", "d"] }),
    )
    .await;
    let voters = seed_members!(server.state.pool(), chat, VOTERS);
    let mut socket = open_socket(base, chat, &alice).await;

    let started = Instant::now();
    let votes = voters.iter().enumerate().map(|(index, voter)| {
        let state = server.state.clone();
        let voter = *voter;
        tokio::spawn(async move { cast_vote(&state, poll, voter, &[(index % 4) as u32]).await })
    });
    for vote in futures_util::future::join_all(votes).await {
        vote.unwrap().expect("every vote succeeds");
    }
    let voting = started.elapsed();

    let frames = drain_type(&mut socket, "poll_updated", POLL_BROADCAST_WINDOW * 4).await;
    let window = POLL_BROADCAST_WINDOW.as_millis();
    // One flush per window while votes land, plus the trailing flush.
    let bound = (voting.as_millis() / window) as usize + 2;
    eprintln!(
        "{VOTERS} votes in {voting:?} -> {} poll_updated frames (bound {bound})",
        frames.len()
    );
    assert!(!frames.is_empty());
    assert!(
        frames.len() <= bound,
        "{} broadcasts for {VOTERS} votes over {voting:?} exceeds {bound}",
        frames.len()
    );
    assert!(frames.len() < VOTERS / 10, "coalescing must dominate");

    let last: Value = serde_json::from_str(frames.last().unwrap()).unwrap();
    assert_eq!(last["poll"]["total_voters"], VOTERS);
    let per_option: Vec<i64> = last["poll"]["options"]
        .as_array()
        .unwrap()
        .iter()
        .map(|option| option["voters"].as_i64().unwrap())
        .collect();
    assert_eq!(per_option, vec![250, 250, 250, 250]);

    drop(socket);
    drop(server);
    for suffix in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{suffix}", database.display()));
    }
    tokio::time::sleep(Duration::from_millis(10)).await;
}
