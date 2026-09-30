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

async fn create_chat(base: &str, name: &str, password: Option<&str>) -> (String, bool) {
    let client = reqwest::Client::new();
    let owner_token = session_token(base, &format!("owner-{name}")).await;
    let body = serde_json::json!({
        "name": name,
        "password": password.unwrap_or("")
    });
    let resp = client
        .post(format!("{}/api/chats", base))
        .bearer_auth(owner_token)
        .json(&body)
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        201,
        "create_chat failed: {:?}",
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
    loop {
        match stream.next().await {
            Some(Ok(Message::Text(t))) => {
                let value = serde_json::from_str::<serde_json::Value>(&t).unwrap();
                if value["type"] != "history_complete" {
                    return value;
                }
            }
            other => panic!("expected text frame, got {:?}", other),
        }
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
async fn create_and_list_chats() {
    let base = start_server().await;

    let list: Vec<serde_json::Value> = reqwest::get(format!("{}/api/chats", base))
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(list.is_empty());

    let id1 = create_chat(&base, "general", Some("pw1")).await.0;
    let id2 = create_chat(&base, "random", None).await.0;
    assert_ne!(id1, id2);

    // The chat list is membership-only, including for anonymous visitors.
    let list: Vec<serde_json::Value> = reqwest::get(format!("{}/api/chats", base))
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(list.is_empty());

    let discover: Vec<serde_json::Value> = reqwest::get(format!("{}/api/chats/discover", base))
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(discover.len(), 1);
    assert_eq!(discover[0]["title"], "random");

    // An authenticated member sees only chats they actually joined.
    let owner_token = session_token(&base, "owner-general").await;
    let authed_list: Vec<serde_json::Value> = reqwest::Client::new()
        .get(format!("{}/api/chats", base))
        .bearer_auth(owner_token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(authed_list.len(), 1);
    assert_eq!(authed_list[0]["title"], "general");
}

/// Regression test: the chat struct cached at creation time carries the
/// creator's own membership_status/membership_role (see handlers::create_chat),
/// so an unrelated user's decorated view must never inherit those stale
/// values — otherwise every chat looks joined to everyone, and a private
/// chat's password-gate gets silently bypassed by the listing endpoint.
#[tokio::test]
async fn list_chats_does_not_leak_creator_membership_to_other_users() {
    let base = start_server().await;
    let (public_id, _) = create_chat(&base, "leak-check-public", None).await;
    let (private_id, _) = create_chat(&base, "leak-check-private", Some("pw")).await;

    let other_token = session_token(&base, "bystander").await;
    let list: Vec<serde_json::Value> = reqwest::Client::new()
        .get(format!("{}/api/chats", base))
        .bearer_auth(other_token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    assert!(
        list.iter().all(|chat| chat["id"] != public_id),
        "a public chat the caller never joined belongs in discovery, not the chat list"
    );

    assert!(
        list.iter().all(|chat| chat["id"] != private_id),
        "a private chat the caller never joined must not appear in the listing at all"
    );
}

#[tokio::test]
async fn public_chat_has_password_false() {
    let base = start_server().await;
    let (id, has_password) = create_chat(&base, "lobby", None).await;
    assert!(!has_password, "public chat should have has_password=false");

    let resp = reqwest::get(format!("{}/api/chats/{}", base, id))
        .await
        .unwrap();
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(!body["has_password"].as_bool().unwrap());
}

#[tokio::test]
async fn private_chat_has_password_true() {
    let base = start_server().await;
    let (id, has_password) = create_chat(&base, "vip", Some("secret")).await;
    assert!(has_password);

    let resp = reqwest::get(format!("{}/api/chats/{}", base, id))
        .await
        .unwrap();
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body["has_password"].as_bool().unwrap());
}

#[tokio::test]
async fn reject_invalid_chat_inputs() {
    let server = start_server().await;
    let client = reqwest::Client::new();
    let url = format!("{}/api/chats", server);
    let token = session_token(&server, "invalid-chat-owner").await;

    for body in [
        serde_json::json!({ "name": "   ", "password": "" }),
        serde_json::json!({ "name": "x".repeat(81), "password": "" }),
        serde_json::json!({ "name": "chat", "password": "x".repeat(257) }),
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
    create_chat(&base, "lobby", None).await;
    let token = session_token(&base, "owner-lobby").await;

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{}/api/chats", base))
        .bearer_auth(token)
        .json(&serde_json::json!({ "name": "lobby", "password": "" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 409);
}

#[tokio::test]
async fn get_chat_by_id() {
    let base = start_server().await;
    let (id, _) = create_chat(&base, "mychat", Some("secret")).await;

    let resp = reqwest::get(format!("{}/api/chats/{}", base, id))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["title"], "mychat");
    assert!(body.get("password_hash").is_none());

    let resp = reqwest::get(format!(
        "{}/api/chats/00000000-0000-0000-0000-000000000000",
        base
    ))
    .await
    .unwrap();
    assert_eq!(resp.status(), 404);
}

#[path = "integration/persistence.rs"]
mod persistence;

#[path = "integration/web_client.rs"]
mod web_client;

#[path = "integration/postgres.rs"]
mod postgres;

#[path = "integration/websocket.rs"]
mod websocket;
