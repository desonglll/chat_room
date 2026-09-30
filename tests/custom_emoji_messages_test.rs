//! TG-304: message entities travel with a text message — over the WebSocket in both
//! directions, in history, through an edit, and disappear with a recall.

mod custom_emoji_support;
mod migration_support;
mod sticker_support;

use custom_emoji_support::{on_postgres, on_sqlite};
use futures_util::{SinkExt, StreamExt};
use reqwest::StatusCode;
use serde_json::{json, Value};
use sticker_support::{http, webp_lossless};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use uuid::Uuid;

type Socket =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

async fn next_json(socket: &mut Socket) -> Value {
    loop {
        if let Message::Text(text) = socket.next().await.unwrap().unwrap() {
            return serde_json::from_str(&text).unwrap();
        }
    }
}

async fn connect(base: &str, room_id: &str, token: &str) -> Socket {
    let url = format!("{}/ws/{room_id}", base.replacen("http://", "ws://", 1));
    let (mut socket, _) = connect_async(url).await.unwrap();
    send(&mut socket, json!({ "type": "join", "token": token })).await;
    loop {
        if next_json(&mut socket).await["type"] == "history_complete" {
            return socket;
        }
    }
}

async fn send(socket: &mut Socket, frame: Value) {
    socket.send(Message::Text(frame.to_string())).await.unwrap();
}

async fn next_of(socket: &mut Socket, kind: &str) -> Value {
    loop {
        let frame = tokio::time::timeout(std::time::Duration::from_secs(5), next_json(socket))
            .await
            .unwrap_or_else(|_| panic!("{kind} within 5 s"));
        if frame["type"] == kind {
            return frame;
        }
    }
}

/// An owner with a custom emoji set holding one 100×100 WebP "😺"; returns (token, emoji id).
async fn seeded_custom_emoji(
    server: &http::Server,
    owner: &str,
    short_name: &str,
) -> (String, Value) {
    let token = server.token(owner).await;
    server.create_set(&token, short_name, "custom_emoji").await;
    let (status, emoji) = server
        .upload(&token, short_name, webp_lossless(100, 100), "😺")
        .await;
    assert_eq!(status, StatusCode::CREATED, "{emoji}");
    (token, emoji["id"].clone())
}

/// Edits reach a socket twice (the inbound broadcast and the edit poller), so match content.
async fn next_edit(socket: &mut Socket, content: &str) -> Value {
    loop {
        let frame = next_of(socket, "message_edited").await;
        if frame["content"] == content {
            return frame;
        }
    }
}

fn message_by_id<'a>(history: &'a Value, id: &Value) -> &'a Value {
    history
        .as_array()
        .unwrap()
        .iter()
        .find(|message| &message["id"] == id)
        .expect("message in history")
}

#[tokio::test]
async fn entities_flow_through_send_history_edit_and_recall_sqlite() {
    on_sqlite(entities_flow).await;
}

#[tokio::test]
async fn entities_flow_through_send_history_edit_and_recall_postgres() {
    on_postgres(
        "entities_flow_through_send_history_edit_and_recall_postgres",
        entities_flow,
    )
    .await;
}

async fn entities_flow(server: http::Server) {
    let (alice, emoji_id) = seeded_custom_emoji(&server, "entity-alice", "entity_cats").await;
    let bob = server.token("entity-bob").await;
    let room_id = server.create_chat(&alice, "entity-room").await;
    let mut alice_socket = connect(&server.base, &room_id, &alice).await;
    let mut bob_socket = connect(&server.base, &room_id, &bob).await;

    // Offsets are UTF-16 units of the raw text; the server trims the two leading spaces and
    // shifts. The unknown custom emoji is dropped, the message and the other entities kept.
    send(
        &mut alice_socket,
        json!({ "type": "message", "content": "  hi 😺!", "client_message_id": Uuid::new_v4(),
            "entities": [
                { "type": "bold", "offset": 2, "length": 2 },
                { "type": "custom_emoji", "offset": 5, "length": 2, "custom_emoji_id": emoji_id },
                { "type": "custom_emoji", "offset": 7, "length": 1, "custom_emoji_id": Uuid::new_v4() },
                { "type": "marquee", "offset": 2, "length": 1 }
            ] }),
    )
    .await;
    let expected = json!([
        { "type": "bold", "offset": 0, "length": 2 },
        { "type": "custom_emoji", "offset": 3, "length": 2, "custom_emoji_id": emoji_id }
    ]);
    let frame = next_of(&mut bob_socket, "broadcast").await;
    assert_eq!(frame["content"], "hi 😺!");
    assert_eq!(frame["entities"], expected);
    let message_id = frame["message_id"].clone();

    // A plain message keeps the old wire shape: no `entities` key at all.
    send(
        &mut alice_socket,
        json!({ "type": "message", "content": "plain" }),
    )
    .await;
    let plain = next_of(&mut bob_socket, "broadcast").await;
    assert!(plain.get("entities").is_none(), "{plain}");

    let (status, history) = server.history(&bob, &room_id).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(message_by_id(&history, &message_id)["entities"], expected);
    assert!(message_by_id(&history, &plain["message_id"])
        .get("entities")
        .is_none());

    // An edit replaces the entity list and `message_edited` carries the new one.
    send(
        &mut alice_socket,
        json!({ "type": "edit", "message_id": message_id, "content": "😺",
            "entities": [{ "type": "custom_emoji", "offset": 0, "length": 2, "custom_emoji_id": emoji_id }] }),
    )
    .await;
    let edited = next_edit(&mut bob_socket, "😺").await;
    assert_eq!(edited["content"], "😺");
    let only_emoji =
        json!([{ "type": "custom_emoji", "offset": 0, "length": 2, "custom_emoji_id": emoji_id }]);
    assert_eq!(edited["entities"], only_emoji);
    let (_, history) = server.history(&bob, &room_id).await;
    assert_eq!(message_by_id(&history, &message_id)["entities"], only_emoji);

    // An edit without entities clears them.
    send(
        &mut alice_socket,
        json!({ "type": "edit", "message_id": message_id, "content": "😺 plain now" }),
    )
    .await;
    let cleared = next_edit(&mut bob_socket, "😺 plain now").await;
    assert!(cleared.get("entities").is_none(), "{cleared}");
    let (_, history) = server.history(&bob, &room_id).await;
    assert!(message_by_id(&history, &message_id)
        .get("entities")
        .is_none());

    // A recalled message shows another viewer neither its text nor its entities.
    send(
        &mut alice_socket,
        json!({ "type": "edit", "message_id": message_id, "content": "😺.",
            "entities": [{ "type": "custom_emoji", "offset": 0, "length": 2, "custom_emoji_id": emoji_id }] }),
    )
    .await;
    next_edit(&mut bob_socket, "😺.").await;
    send(
        &mut alice_socket,
        json!({ "type": "recall", "message_id": message_id }),
    )
    .await;
    next_of(&mut bob_socket, "message_recalled").await;
    let (_, history) = server.history(&bob, &room_id).await;
    let recalled = message_by_id(&history, &message_id);
    assert_eq!(recalled["content"], "");
    assert!(recalled.get("entities").is_none(), "{recalled}");
    let (_, own) = server.history(&alice, &room_id).await;
    assert_eq!(message_by_id(&own, &message_id)["entities"], only_emoji);
}

#[tokio::test]
async fn a_removed_custom_emoji_cannot_be_sent_but_the_text_still_is() {
    on_sqlite(removed_emoji_flow).await;
}

async fn removed_emoji_flow(server: http::Server) {
    let (alice, emoji_id) = seeded_custom_emoji(&server, "entity-removed", "entity_gone").await;
    let room_id = server.create_chat(&alice, "entity-removed-room").await;
    let response = server
        .client
        .delete(server.url(&format!(
            "/api/sticker-sets/entity_gone/stickers/{}",
            emoji_id.as_str().unwrap()
        )))
        .bearer_auth(&alice)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    let mut socket = connect(&server.base, &room_id, &alice).await;
    send(
        &mut socket,
        json!({ "type": "message", "content": "😺",
            "entities": [{ "type": "custom_emoji", "offset": 0, "length": 2, "custom_emoji_id": emoji_id }] }),
    )
    .await;
    let frame = next_of(&mut socket, "broadcast").await;
    assert_eq!(frame["content"], "😺");
    assert!(frame.get("entities").is_none(), "{frame}");
}
