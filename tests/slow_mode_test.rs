//! TG-207 slow mode: the server is the only authority. A member's second message inside the
//! interval is refused on the send path itself (no client involved), owners/admins are exempt,
//! the remaining wait is reported, and only holders of `members.ban` may change the interval.

use std::sync::Arc;
use std::time::Duration;

use chat_room::state::AppState;
use reqwest::{Method, StatusCode};
use serde_json::json;

mod chat_admin_support;

use chat_admin_support::{next_frame, send_text, serve, with_postgres};

async fn slow_mode_scenario(state: Arc<AppState>) {
    let server = serve(state).await;
    let owner = server.register("slow-owner").await;
    let member = server.register("slow-member").await;
    let chat = server.create_group(&owner, "slow").await;
    server.join(&chat, &member).await;
    let path = format!("/api/chats/{chat}/slow-mode");

    // Only the allowed intervals, and only for holders of members.ban.
    let (status, _) = server
        .call(
            Method::PUT,
            &path,
            &owner.token,
            Some(json!({ "seconds": 7 })),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = server
        .call(
            Method::PUT,
            &path,
            &member.token,
            Some(json!({ "seconds": 60 })),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, chat_view) = server
        .call(
            Method::PUT,
            &path,
            &owner.token,
            Some(json!({ "seconds": 60 })),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{chat_view}");
    assert_eq!(chat_view["slow_mode_seconds"], 60);
    assert_eq!(
        chat_view["chat_type"], "supergroup",
        "slow mode upgrades a group"
    );

    // The member's first message goes out; the second, inside the interval, does not.
    let mut member_socket = server.socket(&chat, &member).await;
    send_text(&mut member_socket, "first").await;
    let echoed = next_frame(&mut member_socket, "broadcast").await;
    assert_eq!(echoed["content"], "first");
    send_text(&mut member_socket, "second").await;
    tokio::time::sleep(Duration::from_millis(400)).await;
    assert!(server.history_contains(&chat, &member.token, "first").await);
    assert!(
        !server
            .history_contains(&chat, &member.token, "second")
            .await,
        "the server refused the second message inside the interval"
    );
    let (_, waiting) = server.get(&path, &member.token).await;
    assert_eq!(waiting["seconds"], 60);
    assert_eq!(waiting["exempt"], false);
    let wait = waiting["wait_seconds"].as_i64().unwrap();
    assert!((55..=60).contains(&wait), "remaining wait {wait}");

    // The owner is exempt.
    let mut owner_socket = server.socket(&chat, &owner).await;
    send_text(&mut owner_socket, "owner one").await;
    send_text(&mut owner_socket, "owner two").await;
    tokio::time::sleep(Duration::from_millis(400)).await;
    assert!(
        server
            .history_contains(&chat, &owner.token, "owner one")
            .await
    );
    assert!(
        server
            .history_contains(&chat, &owner.token, "owner two")
            .await
    );
    let (_, exempt) = server.get(&path, &owner.token).await;
    assert_eq!(exempt["exempt"], true);
    assert_eq!(exempt["wait_seconds"], 0);

    // Turning it off lets the member post again at once.
    let (status, _) = server
        .call(
            Method::PUT,
            &path,
            &owner.token,
            Some(json!({ "seconds": 0 })),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    send_text(&mut member_socket, "third").await;
    tokio::time::sleep(Duration::from_millis(400)).await;
    assert!(server.history_contains(&chat, &member.token, "third").await);
}

#[tokio::test]
async fn sqlite_slow_mode_is_enforced_by_the_server() {
    slow_mode_scenario(Arc::new(AppState::new().await.unwrap())).await;
}

#[tokio::test]
async fn postgres_slow_mode_is_enforced_by_the_server() {
    with_postgres("postgres_slow_mode", slow_mode_scenario).await;
}
