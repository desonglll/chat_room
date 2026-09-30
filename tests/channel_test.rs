//! TG-202 acceptance: channel creation, silent lightweight subscriptions, the subscriber
//! count, and signatures. The post gate and moderation keys: `channel_post_gate_test.rs`.

mod channel_support;
mod poll_support;

use std::time::Duration;

use channel_support::*;
use reqwest::{Method, StatusCode};
use serde_json::{json, Value};
use uuid::Uuid;

#[tokio::test]
async fn a_channel_is_created_and_other_types_are_refused() {
    let server = serve_memory().await;
    let base = &server.base;
    let owner = register(base, "ch-create-owner").await;
    let channel = create_channel(base, &owner, "news", true).await;
    assert_eq!(channel["chat_type"], "channel");
    assert_eq!(channel["signatures_enabled"], true);
    assert_eq!(channel["membership_role"], "owner");
    assert_eq!(
        get_chat(base, &owner, id_of(&channel)).await["member_count"],
        1
    );

    for chat_type in ["supergroup", "private", "megagroup"] {
        let (status, _) = call(
            Method::POST,
            format!("{base}/api/chats"),
            &owner.token,
            Some(json!({ "title": format!("t-{chat_type}"), "chat_type": chat_type })),
        )
        .await;
        assert!(status.is_client_error(), "{chat_type}: {status}");
    }
    // A group ignores the signature switch.
    let (status, group) = call(
        Method::POST,
        format!("{base}/api/chats"),
        &owner.token,
        Some(json!({ "title": "plain", "signatures_enabled": true })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(group["chat_type"], "group");
    assert_eq!(group["signatures_enabled"], false);
}

#[tokio::test]
async fn subscribing_is_silent_and_moves_the_subscriber_count() {
    let server = serve_memory().await;
    let base = &server.base;
    let owner = register(base, "ch-sub-owner").await;
    let bob = register(base, "ch-sub-bob").await;
    let carol = register(base, "ch-sub-carol").await;
    let channel = id_of(&create_channel(base, &owner, "silent", false).await);
    let mut owner_socket = open_socket(base, channel, &owner).await;

    let (status, membership) = subscribe(base, &bob, channel).await;
    assert_eq!(status, StatusCode::OK, "{membership}");
    assert_eq!(membership["status"], "active");
    assert_eq!(membership["role"], "member");
    assert_eq!(
        subscribe(base, &bob, channel).await.0,
        StatusCode::OK,
        "idempotent"
    );
    assert_eq!(subscribe(base, &carol, channel).await.0, StatusCode::OK);
    assert_eq!(get_chat(base, &owner, channel).await["member_count"], 3);

    // A subscriber's socket: staff-only participants, no presence/join frame for anyone.
    let url = format!("{}/ws/{channel}", base.replacen("http://", "ws://", 1));
    let (mut bob_socket, _) = tokio_tungstenite::connect_async(url).await.unwrap();
    send_frame(
        &mut bob_socket,
        json!({ "type": "join", "token": bob.token }),
    )
    .await;
    let auth_ok = next_type(&mut bob_socket, "auth_ok").await;
    let participants: Vec<&str> = auth_ok["participants"]
        .as_array()
        .unwrap()
        .iter()
        .map(|member| member["username"].as_str().unwrap())
        .collect();
    assert_eq!(participants, vec!["ch-sub-owner"]);
    assert!(auth_ok["read_receipts"].as_array().unwrap().is_empty());
    next_type(&mut bob_socket, "history_complete").await;

    assert_eq!(
        unsubscribe(base, &carol, channel).await,
        StatusCode::NO_CONTENT
    );
    assert_eq!(get_chat(base, &owner, channel).await["member_count"], 2);
    assert_eq!(
        unsubscribe(base, &owner, channel).await,
        StatusCode::CONFLICT
    );

    for kind in ["system", "presence", "user_status"] {
        let frames = drain_type(&mut owner_socket, kind, Duration::from_millis(300)).await;
        assert!(frames.is_empty(), "a channel broadcast {kind}: {frames:?}");
    }

    // Subscribing is for channels only.
    let group = create_chat(base, &owner, "not-a-channel").await;
    assert_eq!(subscribe(base, &bob, group).await.0, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn signatures_sign_posts_while_switched_on() {
    let server = serve_memory().await;
    let base = &server.base;
    let owner = register(base, "ch-sign-owner").await;
    let bob = register(base, "ch-sign-bob").await;
    let channel = id_of(&create_channel(base, &owner, "signed", true).await);
    subscribe(base, &bob, channel).await;
    let mut owner_socket = open_socket(base, channel, &owner).await;
    say(&mut owner_socket, "signed post").await;
    let signed = next_type(&mut owner_socket, "broadcast").await;
    assert_eq!(signed["post_author"], "ch-sign-owner");

    assert_eq!(
        switch(base, channel, &bob, false).await.0,
        StatusCode::FORBIDDEN
    );
    let (status, chat) = switch(base, channel, &owner, false).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(chat["signatures_enabled"], false);
    next_type(&mut owner_socket, "chat_updated").await;
    say(&mut owner_socket, "unsigned post").await;
    let unsigned = next_type(&mut owner_socket, "broadcast").await;
    assert!(unsigned.get("post_author").is_none(), "{unsigned}");

    // History carries the frozen signature.
    let history = history(base, &bob, channel).await;
    let signed_row = history
        .iter()
        .find(|message| message["content"] == "signed post")
        .unwrap();
    assert_eq!(signed_row["post_author"], "ch-sign-owner");
    assert_eq!(signed_row["sender"], "signed");
}

async fn switch(
    base: &str,
    channel: Uuid,
    account: &Account,
    enabled: bool,
) -> (StatusCode, Value) {
    call(
        Method::PATCH,
        format!("{base}/api/chats/{channel}/channel"),
        &account.token,
        Some(json!({ "signatures_enabled": enabled })),
    )
    .await
}
