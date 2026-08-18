//! Integration tests — REST API, WebSocket (public + private), and persistence.

use chat_room::{build_app, build_app_with_web, state::AppState};
use futures_util::{SinkExt, StreamExt};
use std::{
    fmt,
    ops::Deref,
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::{net::TcpListener, task::JoinHandle};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

mod support;
use support::session_token;

struct TestServer {
    base: String,
    task: Option<JoinHandle<()>>,
}

impl Deref for TestServer {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl fmt::Display for TestServer {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.base.fmt(formatter)
    }
}

impl TestServer {
    async fn shutdown(mut self) {
        if let Some(task) = self.task.take() {
            task.abort();
            let _ = task.await;
        }
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

async fn start_server() -> TestServer {
    let state = Arc::new(AppState::new().await.unwrap());
    start_server_with_state(state).await
}

async fn start_server_with_state(state: Arc<AppState>) -> TestServer {
    start_server_with_app(build_app(state)).await
}

async fn start_web_server() -> TestServer {
    let state = Arc::new(AppState::new().await.unwrap());
    start_server_with_app(build_app_with_web(state, true)).await
}

async fn start_server_with_app(app: axum::Router) -> TestServer {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let task = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    TestServer {
        base: format!("http://127.0.0.1:{}", port),
        task: Some(task),
    }
}

fn temp_path(prefix: &str, extension: &str) -> PathBuf {
    std::env::temp_dir().join(format!("{}-{}.{}", prefix, uuid::Uuid::new_v4(), extension))
}

fn remove_sqlite_files(path: &Path) {
    let _ = std::fs::remove_file(path);
    let _ = std::fs::remove_file(format!("{}-wal", path.display()));
    let _ = std::fs::remove_file(format!("{}-shm", path.display()));
}

async fn create_room(base: &str, name: &str, password: Option<&str>) -> (String, bool) {
    let client = reqwest::Client::new();
    let owner_token = session_token(base, &format!("owner-{name}")).await;
    let body = serde_json::json!({
        "name": name,
        "password": password.unwrap_or("")
    });
    let resp = client
        .post(format!("{}/api/rooms", base))
        .bearer_auth(owner_token)
        .json(&body)
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        201,
        "create_room failed: {:?}",
        resp.text().await
    );
    let body: serde_json::Value = resp.json().await.unwrap();
    let id = body["id"].as_str().unwrap().to_string();
    let has_password = body["has_password"].as_bool().unwrap_or(false);
    (id, has_password)
}

async fn read_json(
    stream: &mut futures_util::stream::SplitStream<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
    >,
) -> serde_json::Value {
    match stream.next().await {
        Some(Ok(Message::Text(t))) => serde_json::from_str::<serde_json::Value>(&t).unwrap(),
        other => panic!("expected text frame, got {:?}", other),
    }
}

async fn read_until_content(
    stream: &mut futures_util::stream::SplitStream<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
    >,
    expected: &str,
) -> serde_json::Value {
    for _ in 0..8 {
        let message = read_json(stream).await;
        if message["content"]
            .as_str()
            .is_some_and(|content| content.contains(expected))
        {
            return message;
        }
    }
    panic!("did not receive a message containing {expected:?}");
}

async fn read_until_type(
    stream: &mut futures_util::stream::SplitStream<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
    >,
    expected: &str,
) -> serde_json::Value {
    for _ in 0..8 {
        let message = read_json(stream).await;
        if message["type"] == expected {
            return message;
        }
    }
    panic!("did not receive a message of type {expected:?}");
}

async fn ws_connect(
    base: &str,
    room_id: &str,
    username: &str,
    password: Option<&str>,
) -> (
    futures_util::stream::SplitSink<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        Message,
    >,
    futures_util::stream::SplitStream<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
    >,
) {
    let ws_base = base.replace("http://", "ws://");
    let url = format!("{}/ws/{}", ws_base, room_id);
    let (ws, _) = connect_async(&url).await.unwrap();
    let (mut sink, mut stream) = ws.split();
    let token = session_token(base, username).await;

    let first = if let Some(pw) = password {
        serde_json::json!({ "type": "auth", "token": token, "password": pw })
    } else {
        serde_json::json!({ "type": "join", "token": token })
    };
    sink.send(Message::Text(first.to_string())).await.unwrap();

    let raw = match stream.next().await {
        Some(Ok(Message::Text(t))) => t.to_string(),
        other => panic!("expected auth response, got {:?}", other),
    };
    let resp: serde_json::Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(resp["type"], "auth_ok", "auth/join failed: {}", resp);

    (sink, stream)
}

// ── REST API tests ──────────────────────────────────────────────────────────

#[tokio::test]
async fn create_and_list_rooms() {
    let base = start_server().await;

    let list: Vec<serde_json::Value> = reqwest::get(format!("{}/api/rooms", base))
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(list.is_empty());

    let id1 = create_room(&base, "general", Some("pw1")).await.0;
    let id2 = create_room(&base, "random", None).await.0;
    assert_ne!(id1, id2);

    let list: Vec<serde_json::Value> = reqwest::get(format!("{}/api/rooms", base))
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(list.len(), 2);
}

#[tokio::test]
async fn public_room_has_password_false() {
    let base = start_server().await;
    let (id, has_password) = create_room(&base, "lobby", None).await;
    assert!(!has_password, "public room should have has_password=false");

    let resp = reqwest::get(format!("{}/api/rooms/{}", base, id))
        .await
        .unwrap();
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(!body["has_password"].as_bool().unwrap());
}

#[tokio::test]
async fn private_room_has_password_true() {
    let base = start_server().await;
    let (id, has_password) = create_room(&base, "vip", Some("secret")).await;
    assert!(has_password);

    let resp = reqwest::get(format!("{}/api/rooms/{}", base, id))
        .await
        .unwrap();
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body["has_password"].as_bool().unwrap());
}

#[tokio::test]
async fn reject_invalid_room_inputs() {
    let server = start_server().await;
    let client = reqwest::Client::new();
    let url = format!("{}/api/rooms", server);
    let token = session_token(&server, "invalid-room-owner").await;

    for body in [
        serde_json::json!({ "name": "   ", "password": "" }),
        serde_json::json!({ "name": "x".repeat(81), "password": "" }),
        serde_json::json!({ "name": "room", "password": "x".repeat(257) }),
    ] {
        let response = client
            .post(&url)
            .bearer_auth(&token)
            .json(&body)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 400);
    }
}

#[tokio::test]
async fn reject_duplicate_room_name() {
    let base = start_server().await;
    create_room(&base, "lobby", None).await;
    let token = session_token(&base, "owner-lobby").await;

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{}/api/rooms", base))
        .bearer_auth(token)
        .json(&serde_json::json!({ "name": "lobby", "password": "" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 409);
}

#[tokio::test]
async fn get_room_by_id() {
    let base = start_server().await;
    let (id, _) = create_room(&base, "mychat", Some("secret")).await;

    let resp = reqwest::get(format!("{}/api/rooms/{}", base, id))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["name"], "mychat");
    assert!(body.get("password_hash").is_none());

    let resp = reqwest::get(format!(
        "{}/api/rooms/00000000-0000-0000-0000-000000000000",
        base
    ))
    .await
    .unwrap();
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn ws_auth_wrong_password() {
    let base = start_server().await;
    let (id, _) = create_room(&base, "secret-room", Some("correct")).await;

    let ws_base = base.replace("http://", "ws://");
    let url = format!("{}/ws/{}", ws_base, id);
    let (ws, _) = connect_async(&url).await.unwrap();
    let (mut sink, mut stream) = ws.split();

    let token = session_token(&base, "alice").await;
    let auth = serde_json::json!({ "type": "auth", "token": token, "password": "wrong" });
    sink.send(Message::Text(auth.to_string())).await.unwrap();

    let raw = match stream.next().await {
        Some(Ok(Message::Text(t))) => t.to_string(),
        other => panic!("expected auth_fail, got {:?}", other),
    };
    let resp: serde_json::Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(resp["type"], "auth_fail");
    assert!(resp["reason"].as_str().unwrap().contains("password"));
}

#[tokio::test]
async fn ws_private_join_and_chat() {
    let base = start_server().await;
    let (id, _) = create_room(&base, "private-chat", Some("pw")).await;

    let (mut sink_a, mut stream_a) = ws_connect(&base, &id, "alice", Some("pw")).await;
    let (_sink_b, mut stream_b) = ws_connect(&base, &id, "bob", Some("pw")).await;

    let msg = read_json(&mut stream_a).await;
    assert_eq!(msg["type"], "system");
    assert!(msg["content"].as_str().unwrap().contains("alice"));
    let msg = read_json(&mut stream_a).await;
    assert_eq!(msg["type"], "system");
    assert!(msg["content"].as_str().unwrap().contains("bob"));

    let msg = read_until_content(&mut stream_b, "bob").await;
    assert_eq!(msg["type"], "system");

    sink_a
        .send(Message::Text(
            serde_json::json!({ "type": "message", "content": "Hello Bob!" }).to_string(),
        ))
        .await
        .unwrap();

    let msg = read_until_type(&mut stream_b, "broadcast").await;
    assert_eq!(msg["sender"], "alice");
    assert_eq!(msg["content"], "Hello Bob!");
    let msg = read_until_type(&mut stream_a, "broadcast").await;
    assert_eq!(msg["sender"], "alice");
    assert_eq!(msg["content"], "Hello Bob!");
}

#[tokio::test]
async fn ws_leave_notifies_others() {
    let base = start_server().await;
    let (id, _) = create_room(&base, "room", Some("pw")).await;

    let (_sink_a, mut stream_a) = ws_connect(&base, &id, "alice", Some("pw")).await;
    let (sink_b, stream_b) = ws_connect(&base, &id, "bob", Some("pw")).await;

    let msg = read_json(&mut stream_a).await;
    assert!(msg["content"].as_str().unwrap().contains("alice"));
    let msg = read_json(&mut stream_a).await;
    assert!(msg["content"].as_str().unwrap().contains("bob"));

    drop(sink_b);
    drop(stream_b);

    let msg = read_until_type(&mut stream_a, "presence").await;
    assert_eq!(msg["members"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn ws_nonexistent_room() {
    let base = start_server().await;
    let ws_base = base.replace("http://", "ws://");
    let url = format!("{}/ws/00000000-0000-0000-0000-000000000000", ws_base);
    let (ws, _) = connect_async(&url).await.unwrap();
    let (_, mut stream) = ws.split();

    let raw = match stream.next().await {
        Some(Ok(Message::Text(t))) => t.to_string(),
        other => panic!("expected auth_fail, got {:?}", other),
    };
    let resp: serde_json::Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(resp["type"], "auth_fail");
}

// ── WebSocket tests — public rooms ──────────────────────────────────────────

#[tokio::test]
async fn ws_public_join_no_password() {
    let base = start_server().await;
    let (id, has_password) = create_room(&base, "public-lounge", None).await;
    assert!(!has_password);

    let (_sink, _stream) = ws_connect(&base, &id, "guest", None).await;
}

#[tokio::test]
async fn ws_public_join_with_auth_also_works() {
    let base = start_server().await;
    let (id, _) = create_room(&base, "open", None).await;

    let (_sink, _stream) = ws_connect(&base, &id, "alice", Some("anything")).await;
}

#[tokio::test]
async fn ws_public_join_rejects_if_private() {
    let base = start_server().await;
    let (id, _) = create_room(&base, "vip-room", Some("secret")).await;

    let ws_base = base.replace("http://", "ws://");
    let url = format!("{}/ws/{}", ws_base, id);
    let (ws, _) = connect_async(&url).await.unwrap();
    let (mut sink, mut stream) = ws.split();

    let token = session_token(&base, "alice").await;
    let join = serde_json::json!({ "type": "join", "token": token });
    sink.send(Message::Text(join.to_string())).await.unwrap();

    let raw = match stream.next().await {
        Some(Ok(Message::Text(t))) => t.to_string(),
        other => panic!("expected auth_fail, got {:?}", other),
    };
    let resp: serde_json::Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(resp["type"], "auth_fail");
    assert!(resp["reason"].as_str().unwrap().contains("password"));
}

#[path = "integration/persistence.rs"]
mod persistence;

#[path = "integration/web_client.rs"]
mod web_client;
