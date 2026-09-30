//! TG-409: quotes and cross-chat replies.
//!
//! - a quote is kept only when it really is a slice of the original, with its UTF-16 offset;
//! - editing the original afterwards marks the quote as modified;
//! - a cross-chat reply shows the target chat only the snapshot the sender shared (the quote,
//!   the source sender, the source chat title) — never the rest of the source message — and
//!   a sender who cannot read the source chat gets a plain message instead.

use std::sync::Arc;

use chat_room::state::AppState;
use futures_util::SinkExt;
use serde_json::{json, Value};
use tokio_tungstenite::tungstenite::Message;

mod chat_admin_support;

use chat_admin_support::{next_frame, send_text, serve, with_postgres, Socket};

async fn send(socket: &mut Socket, frame: Value) {
    socket.send(Message::Text(frame.to_string())).await.unwrap();
}

async fn broadcast_with(socket: &mut Socket, content: &str) -> Value {
    loop {
        let frame = next_frame(socket, "broadcast").await;
        if frame["content"] == content {
            return frame;
        }
    }
}

async fn reply_quotes_scenario(state: Arc<AppState>) {
    let server = serve(state).await;
    let alice = server.register("rq-alice").await;
    let bob = server.register("rq-bob").await;
    let carol = server.register("rq-carol").await;
    let dave = server.register("rq-dave").await;
    let source = server.create_group(&alice, "rq-source").await;
    server.join(&source, &bob).await;
    let target = server.create_group(&bob, "rq-target").await;
    server.join(&target, &carol).await;
    server.join(&target, &dave).await;

    let mut alice_socket = server.socket(&source, &alice).await;
    let mut bob_source = server.socket(&source, &bob).await;
    send_text(&mut alice_socket, "top secret plan: launch at dawn").await;
    let original = broadcast_with(&mut bob_source, "top secret plan: launch at dawn").await;
    let original_id = original["message_id"].as_str().unwrap().to_string();

    // Same chat: a real slice is kept with its offset; a fake one is dropped.
    send(
        &mut bob_source,
        json!({ "type": "message", "content": "agreed", "reply_to": original_id,
                "reply_quote": { "text": "launch at dawn", "offset": 17 } }),
    )
    .await;
    let quoted = broadcast_with(&mut bob_source, "agreed").await;
    assert_eq!(
        quoted["reply_to"]["quote"],
        json!({ "text": "launch at dawn", "offset": 17 })
    );
    assert!(quoted["reply_to"].get("quote_modified").is_none());
    send(
        &mut bob_source,
        json!({ "type": "message", "content": "fake", "reply_to": original_id,
                "reply_quote": { "text": "launch at noon" } }),
    )
    .await;
    let unquoted = broadcast_with(&mut bob_source, "fake").await;
    assert_eq!(unquoted["reply_to"]["message_id"], original_id.as_str());
    assert!(unquoted["reply_to"].get("quote").is_none(), "{unquoted}");

    // Editing the original marks the quote as modified in history.
    send(
        &mut alice_socket,
        json!({ "type": "edit", "message_id": original_id, "content": "top secret plan: launch at noon" }),
    )
    .await;
    next_frame(&mut bob_source, "message_edited").await;
    let (_, history) = server
        .get(&format!("/api/chats/{source}/messages"), &bob.token)
        .await;
    let agreed = history
        .as_array()
        .unwrap()
        .iter()
        .find(|message| message["content"] == "agreed")
        .unwrap();
    assert_eq!(agreed["reply_to"]["quote_modified"], true);

    // Cross-chat: bob replies in the target chat to alice's message in the source chat.
    let mut bob_target = server.socket(&target, &bob).await;
    let mut carol_socket = server.socket(&target, &carol).await;
    send(
        &mut bob_target,
        json!({ "type": "message", "content": "see this", "reply_to": original_id,
                "reply_to_chat_id": source, "reply_quote": { "text": "top secret plan" } }),
    )
    .await;
    let cross = broadcast_with(&mut carol_socket, "see this").await;
    let reply = &cross["reply_to"];
    assert_eq!(reply["chat_id"], source.as_str());
    assert_eq!(reply["chat_title"], "rq-source");
    assert_eq!(reply["sender"], "rq-alice");
    assert_eq!(
        reply["content"], "top secret plan",
        "only the shared snippet"
    );
    assert!(
        !cross.to_string().contains("noon"),
        "the rest of the source never reaches the target chat: {cross}"
    );
    // Carol cannot open the source chat.
    let (status, _) = server
        .get(&format!("/api/chats/{source}/messages"), &carol.token)
        .await;
    assert_ne!(status, reqwest::StatusCode::OK);

    // Dave cannot read the source chat, so his cross-chat reply degrades to a plain message.
    let mut dave_socket = server.socket(&target, &dave).await;
    send(
        &mut dave_socket,
        json!({ "type": "message", "content": "peeking", "reply_to": original_id,
                "reply_to_chat_id": source }),
    )
    .await;
    let plain = broadcast_with(&mut carol_socket, "peeking").await;
    assert!(plain["reply_to"].is_null(), "{plain}");
}

#[tokio::test]
async fn sqlite_quotes_and_cross_chat_replies() {
    reply_quotes_scenario(Arc::new(AppState::new().await.unwrap())).await;
}

#[tokio::test]
async fn postgres_quotes_and_cross_chat_replies() {
    with_postgres("postgres_reply_quotes", reply_quotes_scenario).await;
}
