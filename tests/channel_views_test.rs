//! TG-202 acceptance: 1000 subscribers viewing a post at the same time cost a bounded number
//! of `message_views_updated` broadcasts, the last one carries the exact count, and a view
//! counts once per account.

mod channel_support;
mod poll_support;

use std::sync::Arc;
use std::time::Instant;

use channel_support::*;
use chat_room::chats::channel_views::{record_channel_views, VIEW_BROADCAST_WINDOW};
use chat_room::state::AppState;
use reqwest::{Method, StatusCode};
use serde_json::{json, Value};

const VIEWERS: usize = 1000;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_thousand_simultaneous_viewers_cost_a_bounded_number_of_broadcasts() {
    let database =
        std::env::temp_dir().join(format!("chat-room-views-{}.db", uuid::Uuid::new_v4()));
    let server = serve(Arc::new(AppState::open(&database).await.unwrap())).await;
    let base = &server.base;
    let owner = register(base, "views-owner").await;
    let channel = id_of(&create_channel(base, &owner, "views", false).await);
    let viewers = seed_members!(server.state.pool(), channel, VIEWERS);
    // Seeding bypasses the domain; refresh the cached descriptor like a subscription would.
    assert_eq!(
        get_chat(base, &owner, channel).await["member_count"],
        (VIEWERS + 1) as i64
    );
    let mut socket = open_socket(base, channel, &owner).await;
    say(&mut socket, "breaking news").await;
    let second = {
        say(&mut socket, "more news").await;
        next_type(&mut socket, "broadcast").await;
        next_type(&mut socket, "broadcast").await["message_id"]
            .as_str()
            .unwrap()
            .parse::<uuid::Uuid>()
            .unwrap()
    };
    let history = history(base, &owner, channel).await;
    let posts: Vec<uuid::Uuid> = history
        .iter()
        .map(|message| message["id"].as_str().unwrap().parse().unwrap())
        .collect();
    assert_eq!(posts.len(), 2);

    let started = Instant::now();
    let views = viewers.iter().map(|viewer| {
        let state = server.state.clone();
        let viewer = *viewer;
        let posts = posts.clone();
        tokio::spawn(async move { record_channel_views(&state, channel, viewer, &posts).await })
    });
    for view in futures_util::future::join_all(views).await {
        view.unwrap().expect("every view is accepted");
    }
    let viewing = started.elapsed();

    let frames = drain_type(
        &mut socket,
        "message_views_updated",
        VIEW_BROADCAST_WINDOW * 4,
    )
    .await;
    let window = VIEW_BROADCAST_WINDOW.as_millis();
    // One flush per window while views land, plus the trailing flush.
    let bound = (viewing.as_millis() / window) as usize + 2;
    eprintln!(
        "{VIEWERS} viewers x 2 posts in {viewing:?} -> {} message_views_updated frames (bound {bound})",
        frames.len()
    );
    assert!(!frames.is_empty());
    assert!(
        frames.len() <= bound,
        "{} broadcasts for {VIEWERS} viewers over {viewing:?} exceeds {bound}",
        frames.len()
    );
    assert!(frames.len() < VIEWERS / 10, "coalescing must dominate");
    // Each frame lists many posts at once; the last one has the exact totals.
    let mut last_seen = std::collections::HashMap::new();
    for frame in &frames {
        let value: Value = serde_json::from_str(frame).unwrap();
        for entry in value["views"].as_array().unwrap() {
            last_seen.insert(
                entry["message_id"].as_str().unwrap().to_string(),
                entry["views"].as_i64().unwrap(),
            );
        }
    }
    for post in &posts {
        assert_eq!(last_seen[&post.to_string()], VIEWERS as i64, "{post}");
    }

    // Viewing again changes nothing and broadcasts nothing.
    for viewer in viewers.iter().take(50) {
        record_channel_views(&server.state, channel, *viewer, &[second])
            .await
            .unwrap();
    }
    let repeat = drain_type(
        &mut socket,
        "message_views_updated",
        VIEW_BROADCAST_WINDOW * 3,
    )
    .await;
    assert!(repeat.is_empty(), "{repeat:?}");
    let reloaded = history_views(base, &owner, channel).await;
    assert_eq!(reloaded, vec![VIEWERS as i64; 2]);

    drop(socket);
    drop(server);
    for suffix in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{suffix}", database.display()));
    }
}

async fn history_views(base: &str, account: &Account, channel: uuid::Uuid) -> Vec<i64> {
    history(base, account, channel)
        .await
        .iter()
        .map(|message| message["views"].as_i64().unwrap())
        .collect()
}

#[tokio::test]
async fn the_view_report_is_authorized_and_bounded() {
    let server = serve_memory().await;
    let base = &server.base;
    let owner = register(base, "viewrep-owner").await;
    let bob = register(base, "viewrep-bob").await;
    let stranger = register(base, "viewrep-stranger").await;
    let channel = id_of(&create_channel(base, &owner, "view-reports", false).await);
    subscribe(base, &bob, channel).await;
    let mut socket = open_socket(base, channel, &owner).await;
    say(&mut socket, "post").await;
    let post = next_type(&mut socket, "broadcast").await["message_id"].clone();

    let (status, body) = report(base, &bob, channel, json!([post])).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body["views"][0]["views"], 0,
        "the count moves with the next frame"
    );
    let frame = next_type(&mut socket, "message_views_updated").await;
    assert_eq!(frame["views"], json!([{ "message_id": post, "views": 1 }]));

    assert_eq!(
        report(base, &stranger, channel, json!([post])).await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        report(base, &bob, channel, json!([])).await.0,
        StatusCode::BAD_REQUEST
    );
    let too_many: Vec<String> = (0..101).map(|_| uuid::Uuid::new_v4().to_string()).collect();
    assert_eq!(
        report(base, &bob, channel, json!(too_many)).await.0,
        StatusCode::BAD_REQUEST
    );
    // Unknown ids are ignored, not counted.
    let (status, body) = report(base, &bob, channel, json!([uuid::Uuid::new_v4()])).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["views"], json!([]));
    // Groups have no views.
    let group = create_chat(base, &owner, "no-views").await;
    assert_eq!(
        report(base, &owner, group, json!([post])).await.0,
        StatusCode::NOT_FOUND
    );
}

async fn report(
    base: &str,
    account: &Account,
    chat: uuid::Uuid,
    ids: Value,
) -> (StatusCode, Value) {
    call(
        Method::POST,
        format!("{base}/api/chats/{chat}/message-views"),
        &account.token,
        Some(json!({ "message_ids": ids })),
    )
    .await
}
