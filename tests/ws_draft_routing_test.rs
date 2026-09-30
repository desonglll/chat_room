//! TG-008 live transport proof, beside `tests/ws_frame_routing_test.rs`'s variants but driven
//! through the real HTTP endpoint instead of `AppState::broadcast` directly: a `PUT
//! /api/chats/:id/draft` must fan out to every connection of the drafting account and to
//! nobody else, because the handler broadcasts on the chat channel and only the transport
//! filter (`frame_visible_to`, `src/realtime/protocol.rs`) keeps the draft private.

use futures_util::SinkExt;
use tokio_tungstenite::tungstenite::Message;

mod ws_frame_support;
use ws_frame_support::{collect_until, create_chat, open_chat, session_token, start_server};

async fn put_draft(
    base: &str,
    room_id: uuid::Uuid,
    token: &str,
    body: serde_json::Value,
) -> serde_json::Value {
    let response = reqwest::Client::new()
        .put(format!("{base}/api/chats/{room_id}/draft"))
        .bearer_auth(token)
        .json(&body)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    response.json().await.unwrap()
}

#[tokio::test]
async fn a_saved_draft_reaches_every_connection_of_the_account_and_nobody_else() {
    let server = start_server().await;
    let room_id = create_chat(&server.base, "cloud-drafts", "cd-alice").await;
    let alice_token = session_token(&server.base, "cd-alice").await;
    let bob_token = session_token(&server.base, "cd-bob").await;
    let alice_id = ws_frame_support::user_id(&server.state, &alice_token).await;

    // Two devices of the drafting account, one device of a different account, same chat.
    let (mut alice_first, _) = open_chat(&server.base, room_id, &alice_token).await;
    let (mut alice_second, _) = open_chat(&server.base, room_id, &alice_token).await;
    let (mut bob, _) = open_chat(&server.base, room_id, &bob_token).await;

    let saved = put_draft(
        &server.base,
        room_id,
        &alice_token,
        serde_json::json!({ "text": "typed on device one" }),
    )
    .await;

    for socket in [&mut alice_first, &mut alice_second] {
        let (draft, _) = collect_until(socket, "draft_updated").await;
        assert_eq!(draft["user_id"], alice_id.to_string());
        assert_eq!(draft["text"], "typed on device one");
        assert_eq!(draft["reply_to_message_id"], serde_json::Value::Null);
        assert_eq!(draft["topic_id"], serde_json::Value::Null);
        assert_eq!(
            draft["updated_at"], saved["updated_at"],
            "the frame carries the same server-stamped timestamp as the HTTP response"
        );
    }

    // Bob must see the marker without ever seeing the draft.
    server
        .state
        .broadcast(
            room_id,
            chat_room::models::ChatMessage::System {
                content: "marker".into(),
                members: None,
                participants: None,
            },
        )
        .await;
    // Bob's stream still carries his own join `system` frame, so skip until the marker's
    // content, collecting every frame kind seen on the way.
    let mut skipped = Vec::new();
    loop {
        let frame = ws_frame_support::next_json(&mut bob).await;
        if frame["type"] == "system" && frame["content"] == "marker" {
            break;
        }
        skipped.push(frame["type"].as_str().unwrap_or_default().to_string());
    }
    assert!(
        !skipped.iter().any(|kind| kind == "draft_updated"),
        "another account's draft leaked to bob: {skipped:?}"
    );
}

#[tokio::test]
async fn an_idempotent_replay_broadcasts_nothing_and_a_reconnect_reads_the_draft_back() {
    let server = start_server().await;
    let room_id = create_chat(&server.base, "draft-replay", "dr2-alice").await;
    let alice_token = session_token(&server.base, "dr2-alice").await;

    let (mut alice, _) = open_chat(&server.base, room_id, &alice_token).await;
    put_draft(
        &server.base,
        room_id,
        &alice_token,
        serde_json::json!({ "text": "only once" }),
    )
    .await;
    let (first, _) = collect_until(&mut alice, "draft_updated").await;
    assert_eq!(first["text"], "only once");

    // The debounced client retries the identical payload: no second frame may arrive.
    put_draft(
        &server.base,
        room_id,
        &alice_token,
        serde_json::json!({ "text": "only once" }),
    )
    .await;
    server
        .state
        .broadcast(
            room_id,
            chat_room::models::ChatMessage::System {
                content: "after-replay".into(),
                members: None,
                participants: None,
            },
        )
        .await;
    let (marker, skipped) = collect_until(&mut alice, "system").await;
    assert_eq!(marker["content"], "after-replay");
    assert!(
        !skipped.iter().any(|kind| kind == "draft_updated"),
        "an idempotent PUT must not re-broadcast: {skipped:?}"
    );

    // 断线重连后草稿不丢: drop every connection, reconnect, read the draft back over REST.
    alice.send(Message::Close(None)).await.ok();
    drop(alice);
    let (_reconnected, _) = open_chat(&server.base, room_id, &alice_token).await;
    let fetched: serde_json::Value = reqwest::Client::new()
        .get(format!("{}/api/chats/{room_id}/draft", server.base))
        .bearer_auth(&alice_token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(fetched["text"], "only once");
    assert_eq!(fetched["updated_at"], first["updated_at"]);
}
