//! TG-406 poll lifecycle around the message: forwarding copies a poll (Telegram semantics),
//! creation is validated and idempotent, and a recalled poll disappears.

mod poll_support;

use std::time::Duration;

use futures_util::SinkExt;
use poll_support::*;
use reqwest::{Method, StatusCode};
use serde_json::json;
use tokio_tungstenite::tungstenite::Message;

#[tokio::test]
async fn forwarding_a_poll_creates_a_new_poll_without_votes() {
    let server = serve_memory().await;
    let base = &server.base;
    let alice = register(base, "forward-poll-alice").await;
    let bob = register(base, "forward-poll-bob").await;
    let source_chat = create_chat(base, &alice, "forward-source").await;
    let target_chat = create_chat(base, &alice, "forward-target").await;
    join(base, &bob, source_chat).await;
    let source = poll(
        base,
        &alice,
        source_chat,
        json!({ "question": "Keep?", "options": ["yes", "no"], "public_voters": true }),
    )
    .await;
    vote(base, &bob, source, &[0]).await;
    close_poll(base, &alice, source).await;

    let (status, results) = call(
        Method::POST,
        format!("{base}/api/messages/forward"),
        &alice.token,
        Some(json!({ "message_ids": [source], "target_room_ids": [target_chat] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{results}");
    let copy = results[0]["forwarded_message_id"]
        .as_str()
        .unwrap()
        .to_string();
    assert_ne!(copy, source.to_string());

    let (_, copied) = get_poll(base, &alice, &copy).await;
    assert_eq!(copied["id"], copy);
    assert_eq!(copied["question"], "Keep?");
    assert_eq!(copied["public_voters"], true, "mode is copied");
    assert_eq!(copied["closed"], false, "the copy is a fresh, open poll");
    assert_eq!(copied["total_voters"], 0, "votes are not copied");

    let copy_id = copy.parse().unwrap();
    assert_eq!(vote(base, &alice, copy_id, &[1]).await.0, StatusCode::OK);
    let (_, original) = get_poll(base, &alice, source).await;
    assert_eq!(
        original["options"][1]["voters"], 0,
        "the two polls are independent"
    );
    assert_eq!(original["total_voters"], 1);
}

#[tokio::test]
async fn creation_is_validated_idempotent_and_needs_membership() {
    let server = serve_memory().await;
    let base = &server.base;
    let alice = register(base, "create-poll-alice").await;
    let stranger = register(base, "create-poll-stranger").await;
    let chat = create_chat(base, &alice, "create-poll").await;

    for body in [
        json!({ "question": "", "options": ["a", "b"] }),
        json!({ "question": "Q", "options": ["a"] }),
        json!({ "question": "Q", "options": ["a", "b"], "correct_option": 0 }),
        json!({ "question": "Q", "options": ["a", "b"], "quiz": true }),
    ] {
        assert_eq!(
            create_poll(base, &alice, chat, body).await.0,
            StatusCode::BAD_REQUEST
        );
    }
    let body = json!({ "question": "Q", "options": ["a", "b"] });
    assert_eq!(
        create_poll(base, &stranger, chat, body.clone()).await.0,
        StatusCode::NOT_FOUND
    );

    let key = uuid::Uuid::new_v4();
    let keyed = json!({ "question": "Once", "options": ["a", "b"], "client_message_id": key });
    let (first_status, first) = create_poll(base, &alice, chat, keyed.clone()).await;
    let (second_status, second) = create_poll(base, &alice, chat, keyed).await;
    assert_eq!(
        (first_status, second_status),
        (StatusCode::CREATED, StatusCode::CREATED)
    );
    assert_eq!(first["id"], second["id"]);
    let polls = history(base, &alice, chat)
        .await
        .iter()
        .filter(|m| m["content"] == "Once")
        .count();
    assert_eq!(polls, 1);
}

#[tokio::test]
async fn a_recalled_poll_disappears_and_refuses_votes() {
    let server = serve_memory().await;
    let base = &server.base;
    let alice = register(base, "recall-poll-alice").await;
    let bob = register(base, "recall-poll-bob").await;
    let chat = create_chat(base, &alice, "recall-poll").await;
    join(base, &bob, chat).await;
    let poll = poll(
        base,
        &alice,
        chat,
        json!({ "question": "Oops", "options": ["a", "b"] }),
    )
    .await;
    let mut socket = open_socket(base, chat, &alice).await;
    socket
        .send(Message::Text(
            json!({ "type": "recall", "message_id": poll }).to_string(),
        ))
        .await
        .unwrap();
    next_type(&mut socket, "message_recalled").await;
    tokio::time::sleep(Duration::from_millis(50)).await;

    assert_eq!(vote(base, &bob, poll, &[0]).await.0, StatusCode::NOT_FOUND);
    let message = history(base, &bob, chat)
        .await
        .iter()
        .find(|m| m["id"] == poll.to_string())
        .unwrap()
        .clone();
    assert!(
        message.get("poll").is_none(),
        "a recalled poll carries no poll"
    );
}
