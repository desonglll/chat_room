//! TG-405 auto-delete. The timer only stamps messages sent while it is on (so changing or
//! turning it off never touches earlier messages); the sweep deletes expired messages for
//! everyone, tells clients with `messages_deleted`, and is idempotent; only `chat.info` holders
//! may set it in a group.

use std::sync::Arc;
use std::time::Duration;

use chat_room::state::AppState;
use chrono::Utc;
use reqwest::{Method, StatusCode};
use serde_json::json;

mod chat_admin_support;

use chat_admin_support::{next_frame, send_text, serve, with_postgres};

const DAY: i64 = 86_400;

async fn auto_delete_scenario(state: Arc<AppState>) {
    let server = serve(state.clone()).await;
    let owner = server.register("ad-owner").await;
    let member = server.register("ad-member").await;
    let chat = server.create_group(&owner, "ad-group").await;
    server.join(&chat, &member).await;
    let path = format!("/api/chats/{chat}/auto-delete");
    let mut owner_socket = server.socket(&chat, &owner).await;
    let mut member_socket = server.socket(&chat, &member).await;

    send_text(&mut owner_socket, "before the timer").await;
    next_frame(&mut member_socket, "broadcast").await;

    // Only the allowed timers, and only for chat.info holders in a group.
    let (status, _) = server
        .call(
            Method::PUT,
            &path,
            &owner.token,
            Some(json!({ "seconds": 5 })),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = server
        .call(
            Method::PUT,
            &path,
            &member.token,
            Some(json!({ "seconds": DAY })),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, updated) = server
        .call(
            Method::PUT,
            &path,
            &owner.token,
            Some(json!({ "seconds": DAY })),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    assert_eq!(updated["auto_delete_seconds"], DAY);

    send_text(&mut owner_socket, "under the timer").await;
    let stamped = loop {
        let frame = next_frame(&mut member_socket, "broadcast").await;
        if frame["content"] == "under the timer" {
            break frame;
        }
    };

    // Turning the timer off does not rescue messages sent while it was on.
    let (status, _) = server
        .call(
            Method::PUT,
            &path,
            &owner.token,
            Some(json!({ "seconds": 0 })),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    send_text(&mut owner_socket, "after the timer").await;
    tokio::time::sleep(Duration::from_millis(300)).await;

    // Nothing is due yet.
    assert_eq!(
        state.sweep_auto_deleted_messages(Utc::now()).await.unwrap(),
        0
    );

    // A day later only the message sent under the timer is gone, for everyone.
    let later = Utc::now() + chrono::Duration::seconds(DAY + 60);
    assert_eq!(state.sweep_auto_deleted_messages(later).await.unwrap(), 1);
    let deleted = next_frame(&mut member_socket, "messages_deleted").await;
    assert_eq!(deleted["message_ids"], json!([stamped["message_id"]]));
    assert!(
        server
            .history_contains(&chat, &member.token, "before the timer")
            .await
    );
    assert!(
        !server
            .history_contains(&chat, &member.token, "under the timer")
            .await
    );
    assert!(
        server
            .history_contains(&chat, &member.token, "after the timer")
            .await
    );

    // The sweep is idempotent.
    assert_eq!(state.sweep_auto_deleted_messages(later).await.unwrap(), 0);

    // In a private chat either participant may set the timer (direct chats need friends).
    let (status, _) = server
        .call(
            Method::POST,
            "/api/friend-requests",
            &member.token,
            Some(json!({ "user_id": owner.id })),
        )
        .await;
    assert!(status.is_success(), "friend request: {status}");
    let (status, _) = server
        .call(
            Method::PATCH,
            &format!("/api/friend-requests/{}", member.id),
            &owner.token,
            Some(json!({ "action": "accept" })),
        )
        .await;
    assert!(status.is_success(), "accept: {status}");
    let (status, direct) = server
        .call(
            Method::POST,
            "/api/direct-chats",
            &member.token,
            Some(json!({ "user_id": owner.id })),
        )
        .await;
    assert!(status.is_success(), "{status} {direct}");
    let direct_id = direct["room_id"].as_str().unwrap().to_string();
    let (status, _) = server
        .call(
            Method::PUT,
            &format!("/api/chats/{direct_id}/auto-delete"),
            &member.token,
            Some(json!({ "seconds": 604_800 })),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn sqlite_auto_delete_stamps_new_messages_and_sweeps_them() {
    auto_delete_scenario(Arc::new(AppState::new().await.unwrap())).await;
}

#[tokio::test]
async fn postgres_auto_delete_stamps_new_messages_and_sweeps_them() {
    with_postgres("postgres_auto_delete", auto_delete_scenario).await;
}
