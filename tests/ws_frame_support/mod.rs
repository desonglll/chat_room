//! Shared live-server WebSocket helpers for the TG-007 frame tests
//! (`ws_frame_broadcast_test.rs`, `ws_frame_routing_test.rs`).

use std::sync::Arc;
use std::time::Duration;

use chat_room::{build_app, state::AppState};
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use uuid::Uuid;

#[path = "../support/mod.rs"]
mod support;
pub use support::session_token;

pub type Socket =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

pub struct TestServer {
    pub base: String,
    pub state: Arc<AppState>,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

pub async fn start_server() -> TestServer {
    let state = Arc::new(AppState::new().await.unwrap());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let app = build_app(state.clone());
    let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    TestServer {
        base: format!("http://{address}"),
        state,
        task,
    }
}

pub async fn create_chat(base: &str, title: &str, owner: &str) -> Uuid {
    let owner_token = session_token(base, owner).await;
    let id = reqwest::Client::new()
        .post(format!("{base}/api/chats"))
        .bearer_auth(owner_token)
        .json(&serde_json::json!({ "title": title, "password": "", "join_policy": "open" }))
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();
    Uuid::parse_str(&id).unwrap()
}

pub async fn open_chat(base: &str, room_id: Uuid, token: &str) -> (Socket, serde_json::Value) {
    let url = format!("{}/ws/{room_id}", base.replacen("http://", "ws://", 1));
    let (mut socket, _) = connect_async(url).await.unwrap();
    socket
        .send(Message::Text(
            serde_json::json!({ "type": "join", "token": token }).to_string(),
        ))
        .await
        .unwrap();
    let auth = next_json(&mut socket).await;
    assert_eq!(auth["type"], "auth_ok");
    (socket, auth)
}

pub async fn next_json(socket: &mut Socket) -> serde_json::Value {
    loop {
        let frame = tokio::time::timeout(Duration::from_secs(3), socket.next())
            .await
            .expect("timed out waiting for WebSocket frame")
            .expect("WebSocket ended")
            .expect("WebSocket error");
        let Message::Text(text) = frame else { continue };
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        if value["type"] != "history_complete" {
            return value;
        }
    }
}

/// Read frames until one of `expected` type arrives, returning it plus everything skipped.
pub async fn collect_until(
    socket: &mut Socket,
    expected: &str,
) -> (serde_json::Value, Vec<String>) {
    let mut skipped = Vec::new();
    loop {
        let value = next_json(socket).await;
        if value["type"] == expected {
            return (value, skipped);
        }
        skipped.push(value["type"].as_str().unwrap_or_default().to_string());
    }
}

pub async fn user_id(state: &AppState, token: &str) -> Uuid {
    state
        .session_user(Uuid::parse_str(token).unwrap())
        .await
        .unwrap()
        .unwrap()
        .id
}
