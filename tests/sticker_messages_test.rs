//! TG-302: sending a sticker into a chat, and its wire shape in history and on the WebSocket.

mod sticker_support;

use chat_room::config::AppConfig;
use futures_util::{SinkExt, StreamExt};
use reqwest::StatusCode;
use serde_json::{json, Value};
use sticker_support::{http, valid_tgs};
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
    socket
        .send(Message::Text(
            json!({ "type": "join", "token": token }).to_string(),
        ))
        .await
        .unwrap();
    loop {
        if next_json(&mut socket).await["type"] == "history_complete" {
            return socket;
        }
    }
}

async fn next_broadcast(socket: &mut Socket) -> Value {
    loop {
        let frame = tokio::time::timeout(std::time::Duration::from_secs(5), next_json(socket))
            .await
            .expect("broadcast within 5 s");
        if frame["type"] == "broadcast" {
            return frame;
        }
    }
}

#[tokio::test]
async fn a_sticker_message_flows_through_history_and_the_websocket() {
    let server = http::start(AppConfig::default()).await;
    let (owner, sticker) = server.seeded_sticker("sticker-send-owner", "crabs").await;
    let room_id = server.create_chat(&owner, "sticker-room").await;
    let mut socket = connect(&server.base, &room_id, &owner).await;

    let client_message_id = Uuid::new_v4();
    let body = json!({ "sticker_id": sticker["id"], "client_message_id": client_message_id });
    let (status, sent) = server.send_sticker(&owner, &room_id, body.clone()).await;
    assert_eq!(status, StatusCode::CREATED, "{sent}");
    assert_eq!(sent["media_kind"], "sticker");
    assert_eq!(sent["content"], "");
    assert_eq!(
        sent["sticker"],
        json!({ "sticker_id": sticker["id"], "set_id": sticker["set_id"],
                "set_short_name": "crabs", "emoji": "🦀", "format": "tgs",
                "width": 512, "height": 512 })
    );
    assert_eq!(sent["attachment"]["mime_type"], "application/x-tgsticker");
    assert_eq!(sent["attachment"]["file_name"], "sticker.tgs");

    let frame = next_broadcast(&mut socket).await;
    assert_eq!(frame["message_id"], sent["id"]);
    assert_eq!(frame["media_kind"], "sticker");
    assert_eq!(frame["sticker"], sent["sticker"]);
    assert_eq!(frame["attachment"], sent["attachment"]);

    // A retried send with the same client_message_id returns the original, no duplicate.
    let (status, replay) = server.send_sticker(&owner, &room_id, body).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(replay["id"], sent["id"]);

    let (status, history) = server.history(&owner, &room_id).await;
    assert_eq!(status, StatusCode::OK);
    let stickers: Vec<&Value> = history
        .as_array()
        .unwrap()
        .iter()
        .filter(|message| message["media_kind"] == "sticker")
        .collect();
    assert_eq!(stickers.len(), 1);
    assert_eq!(stickers[0]["sticker"], sent["sticker"]);
    // Plain messages keep their exact old shape: no media fields at all.
    assert!(history
        .as_array()
        .unwrap()
        .iter()
        .filter(|message| message["id"] != sent["id"])
        .all(|message| message.get("media_kind").is_none() && message.get("sticker").is_none()));

    let (status, file) = server
        .fetch(sent["attachment"]["download_url"].as_str().unwrap())
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(file, valid_tgs());

    let recent: Value = server
        .client
        .get(server.url("/api/stickers/recent"))
        .bearer_auth(&owner)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        recent[0]["id"], sticker["id"],
        "sending records a recent sticker"
    );

    let (status, missing) = server
        .send_sticker(&owner, &room_id, json!({ "sticker_id": Uuid::new_v4() }))
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(missing["error"], "sticker_not_found");
}
