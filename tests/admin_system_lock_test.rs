use std::{sync::Arc, time::Duration};

use chat_room::{
    build_app,
    config::{AdminConfig, AppConfig},
    state::AppState,
};
use futures_util::{SinkExt, StreamExt};
use reqwest::{Client, StatusCode};
use tokio::net::TcpListener;
use tokio_tungstenite::{connect_async, tungstenite::Message};

mod support;
use support::session_token;

type Socket =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

struct Server {
    base: String,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn start() -> Server {
    let config = AppConfig {
        admin: AdminConfig {
            usernames: vec!["ops-admin".into()],
            ..AdminConfig::default()
        },
        ..AppConfig::default()
    };
    let state = Arc::new(AppState::new_with_config(&config).await.unwrap());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        axum::serve(listener, build_app(state)).await.unwrap();
    });
    Server { base, task }
}

async fn next_json(socket: &mut Socket) -> serde_json::Value {
    loop {
        let frame = tokio::time::timeout(Duration::from_secs(3), socket.next())
            .await
            .expect("timed out waiting for WebSocket frame")
            .expect("WebSocket ended")
            .expect("WebSocket error");
        let Message::Text(text) = frame else { continue };
        return serde_json::from_str(&text).unwrap();
    }
}

async fn next_type(socket: &mut Socket, expected: &str) -> serde_json::Value {
    loop {
        let value = next_json(socket).await;
        if value["type"] == expected {
            return value;
        }
    }
}

async fn open_room(base: &str, room_id: &str, token: &str) -> (Socket, serde_json::Value) {
    let url = format!("{}/ws/{room_id}", base.replacen("http://", "ws://", 1));
    let (mut socket, _) = connect_async(url).await.unwrap();
    socket
        .send(Message::Text(
            serde_json::json!({ "type": "join", "token": token }).to_string(),
        ))
        .await
        .unwrap();
    let response = next_json(&mut socket).await;
    (socket, response)
}

async fn set_lock(client: &Client, base: &str, token: &str, locked: bool) -> reqwest::Response {
    client
        .put(format!("{base}/api/admin/chat-lock"))
        .bearer_auth(token)
        .json(&serde_json::json!({ "locked": locked }))
        .send()
        .await
        .unwrap()
}

async fn set_room_lock(
    client: &Client,
    base: &str,
    token: &str,
    room_id: &str,
    locked: bool,
) -> reqwest::Response {
    client
        .put(format!("{base}/api/admin/room-locks/{room_id}"))
        .bearer_auth(token)
        .json(&serde_json::json!({ "locked": locked }))
        .send()
        .await
        .unwrap()
}

#[tokio::test]
async fn administrators_can_lock_and_unlock_every_chat_room() {
    let server = start().await;
    let client = Client::new();
    let admin = session_token(&server.base, "ops-admin").await;
    let regular = session_token(&server.base, "lock-regular").await;
    let visitor = session_token(&server.base, "lock-visitor").await;
    let room: serde_json::Value = client
        .post(format!("{}/api/rooms", server.base))
        .bearer_auth(&regular)
        .json(&serde_json::json!({
            "name": "lock-test-room",
            "password": "",
            "join_policy": "open"
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let room_id = room["id"].as_str().unwrap();
    let (mut connected, auth) = open_room(&server.base, room_id, &regular).await;
    assert_eq!(auth["type"], "auth_ok");

    assert_eq!(
        client
            .put(format!("{}/api/admin/chat-lock", server.base))
            .json(&serde_json::json!({ "locked": true }))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        set_lock(&client, &server.base, &regular, true)
            .await
            .status(),
        StatusCode::FORBIDDEN
    );

    let locked: serde_json::Value = set_lock(&client, &server.base, &admin, true)
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(locked["locked"], true);
    let disconnected = next_type(&mut connected, "system").await;
    assert_eq!(disconnected["content"], "system locked");

    let (_, rejected) = open_room(&server.base, room_id, &regular).await;
    assert_eq!(rejected["type"], "auth_fail");
    assert_eq!(rejected["reason"], "system locked");
    assert_eq!(
        client
            .post(format!("{}/api/rooms/{room_id}/join-requests", server.base))
            .bearer_auth(&visitor)
            .json(&serde_json::json!({}))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::LOCKED
    );
    assert_eq!(
        client
            .post(format!("{}/api/rooms", server.base))
            .bearer_auth(&regular)
            .json(&serde_json::json!({ "name": "blocked-room", "password": "" }))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::LOCKED
    );
    assert_eq!(
        client
            .post(format!("{}/api/direct-chats", server.base))
            .bearer_auth(&regular)
            .json(&serde_json::json!({
                "user_id": "00000000-0000-0000-0000-000000000000"
            }))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::LOCKED
    );
    let overview: serde_json::Value = client
        .get(format!("{}/api/admin/overview", server.base))
        .bearer_auth(&admin)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(overview["chat_rooms_locked"], true);

    let unlocked: serde_json::Value = set_lock(&client, &server.base, &admin, false)
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(unlocked["locked"], false);
    let (_, restored) = open_room(&server.base, room_id, &regular).await;
    assert_eq!(restored["type"], "auth_ok");
}

#[tokio::test]
async fn administrators_can_lock_one_room_without_affecting_others() {
    let server = start().await;
    let client = Client::new();
    let admin = session_token(&server.base, "ops-admin").await;
    let regular = session_token(&server.base, "room-lock-regular").await;
    let visitor = session_token(&server.base, "room-lock-visitor").await;
    let first: serde_json::Value = client
        .post(format!("{}/api/rooms", server.base))
        .bearer_auth(&regular)
        .json(&serde_json::json!({ "name": "locked-room", "password": "" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let second: serde_json::Value = client
        .post(format!("{}/api/rooms", server.base))
        .bearer_auth(&regular)
        .json(&serde_json::json!({ "name": "open-room", "password": "" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let first_id = first["id"].as_str().unwrap();
    let second_id = second["id"].as_str().unwrap();
    let (mut first_socket, first_auth) = open_room(&server.base, first_id, &regular).await;
    let (_, second_auth) = open_room(&server.base, second_id, &regular).await;
    assert_eq!(first_auth["type"], "auth_ok");
    assert_eq!(second_auth["type"], "auth_ok");

    assert_eq!(
        client
            .get(format!("{}/api/admin/room-locks/{first_id}", server.base))
            .bearer_auth(&regular)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    let initial: serde_json::Value = client
        .get(format!("{}/api/admin/room-locks/{first_id}", server.base))
        .bearer_auth(&admin)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(initial["locked"], false);

    let locked: serde_json::Value = set_room_lock(&client, &server.base, &admin, first_id, true)
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(locked["room_id"], first_id);
    assert_eq!(locked["locked"], true);
    assert_eq!(
        next_type(&mut first_socket, "system").await["content"],
        "room locked"
    );

    let (_, rejected) = open_room(&server.base, first_id, &regular).await;
    assert_eq!(rejected["type"], "auth_fail");
    assert_eq!(rejected["reason"], "room locked");
    let (_, unaffected) = open_room(&server.base, second_id, &regular).await;
    assert_eq!(unaffected["type"], "auth_ok");
    assert_eq!(
        client
            .post(format!(
                "{}/api/rooms/{first_id}/join-requests",
                server.base
            ))
            .bearer_auth(&visitor)
            .json(&serde_json::json!({}))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::LOCKED
    );
    assert!(client
        .post(format!(
            "{}/api/rooms/{second_id}/join-requests",
            server.base
        ))
        .bearer_auth(&visitor)
        .json(&serde_json::json!({}))
        .send()
        .await
        .unwrap()
        .status()
        .is_success());
    assert_eq!(
        client
            .post(format!("{}/api/rooms", server.base))
            .bearer_auth(&regular)
            .json(&serde_json::json!({ "name": "still-open", "password": "" }))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::CREATED
    );

    assert_eq!(
        set_room_lock(&client, &server.base, &admin, first_id, false)
            .await
            .status(),
        StatusCode::OK
    );
    let (_, restored) = open_room(&server.base, first_id, &regular).await;
    assert_eq!(restored["type"], "auth_ok");
}
