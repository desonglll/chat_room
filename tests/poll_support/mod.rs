//! Shared harness for the TG-406 poll tests: a served `AppState`, accounts, chats, JSON calls,
//! WebSocket frames, and bulk member seeding for the load/consistency tests.

#![allow(dead_code)]

use std::sync::Arc;
use std::time::Duration;

use chat_room::{build_app, state::AppState};
use futures_util::{SinkExt, StreamExt};
use reqwest::{Method, StatusCode};
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use uuid::Uuid;

pub type Socket =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

pub struct Server {
    pub base: String,
    pub state: Arc<AppState>,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}

pub async fn serve(state: Arc<AppState>) -> Server {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn({
        let state = state.clone();
        async move { axum::serve(listener, build_app(state)).await.unwrap() }
    });
    Server { base, state, task }
}

pub async fn serve_memory() -> Server {
    serve(Arc::new(AppState::new().await.unwrap())).await
}

#[derive(Clone, Debug)]
pub struct Account {
    pub id: Uuid,
    pub token: String,
}

pub async fn register(base: &str, username: &str) -> Account {
    let value: Value = reqwest::Client::new()
        .post(format!("{base}/api/users/register"))
        .json(&json!({ "username": username, "password": "test-password" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    Account {
        id: value["user"]["id"].as_str().unwrap().parse().unwrap(),
        token: value["token"].as_str().unwrap().into(),
    }
}

/// One JSON request. Returns the status and the body (`Value::Null` when there is none).
pub async fn call(
    method: Method,
    url: String,
    token: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut request = reqwest::Client::new()
        .request(method, url)
        .bearer_auth(token);
    if let Some(body) = body {
        request = request.json(&body);
    }
    let response = request.send().await.unwrap();
    let status = response.status();
    let text = response.text().await.unwrap();
    (status, serde_json::from_str(&text).unwrap_or(Value::Null))
}

pub async fn create_chat(base: &str, owner: &Account, name: &str) -> Uuid {
    let (status, body) = call(
        Method::POST,
        format!("{base}/api/chats"),
        &owner.token,
        Some(json!({ "name": name, "password": "", "join_policy": "open" })),
    )
    .await;
    assert!(status.is_success(), "create chat: {status} {body}");
    body["id"].as_str().unwrap().parse().unwrap()
}

pub async fn join(base: &str, account: &Account, chat: Uuid) {
    let (status, body) = call(
        Method::POST,
        format!("{base}/api/chats/{chat}/join-requests"),
        &account.token,
        Some(json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "join: {body}");
}

pub async fn create_poll(
    base: &str,
    account: &Account,
    chat: Uuid,
    body: Value,
) -> (StatusCode, Value) {
    call(
        Method::POST,
        format!("{base}/api/chats/{chat}/polls"),
        &account.token,
        Some(body),
    )
    .await
}

/// Create a poll that must succeed; returns its message id.
pub async fn poll(base: &str, account: &Account, chat: Uuid, body: Value) -> Uuid {
    let (status, message) = create_poll(base, account, chat, body).await;
    assert_eq!(status, StatusCode::CREATED, "create poll: {message}");
    message["id"].as_str().unwrap().parse().unwrap()
}

pub async fn vote(
    base: &str,
    account: &Account,
    poll: Uuid,
    options: &[u32],
) -> (StatusCode, Value) {
    call(
        Method::POST,
        format!("{base}/api/polls/{poll}/votes"),
        &account.token,
        Some(json!({ "options": options })),
    )
    .await
}

pub async fn open_socket(base: &str, chat: Uuid, account: &Account) -> Socket {
    let url = format!("{}/ws/{chat}", base.replacen("http://", "ws://", 1));
    let (mut socket, _) = connect_async(url).await.unwrap();
    socket
        .send(Message::Text(
            json!({ "type": "join", "token": account.token }).to_string(),
        ))
        .await
        .unwrap();
    next_type(&mut socket, "history_complete").await;
    socket
}

/// The next frame of `expected` type, skipping others. Panics after 5 s of silence.
pub async fn next_type(socket: &mut Socket, expected: &str) -> Value {
    loop {
        let frame = tokio::time::timeout(Duration::from_secs(5), socket.next())
            .await
            .unwrap_or_else(|_| panic!("timed out waiting for a `{expected}` frame"))
            .expect("WebSocket ended")
            .expect("WebSocket error");
        let Message::Text(text) = frame else { continue };
        let value: Value = serde_json::from_str(&text).unwrap();
        if value["type"] == expected {
            return value;
        }
    }
}

/// Every `expected` frame's raw text until the socket stays quiet for `quiet`.
pub async fn drain_type(socket: &mut Socket, expected: &str, quiet: Duration) -> Vec<String> {
    let mut frames = Vec::new();
    while let Ok(Some(Ok(frame))) = tokio::time::timeout(quiet, socket.next()).await {
        if let Message::Text(text) = frame {
            let value: Value = serde_json::from_str(&text).unwrap();
            if value["type"] == expected {
                frames.push(text.to_string());
            }
        }
    }
    frames
}

/// Insert `count` users as active plain members of `chat`, straight into the database (the
/// HTTP registration path hashes a password per account, far too slow for a 1000-voter test).
#[macro_export]
macro_rules! seed_members {
    ($pool:expr, $chat:expr, $count:expr) => {{
        let now = chrono::Utc::now();
        let role = format!("{}:member", $chat.simple());
        let mut ids = Vec::with_capacity($count);
        for _ in 0..$count {
            let id = uuid::Uuid::new_v4();
            sqlx::query(
                "INSERT INTO users (id, username, password_hash, created_at) \
                 VALUES ($1, $2, 'x', $3)",
            )
            .bind(id)
            .bind(format!("voter-{}", id.simple()))
            .bind(now)
            .execute($pool)
            .await
            .unwrap();
            sqlx::query(
                "INSERT INTO chat_members (room_id, user_id, role_id, status, requested_at, \
                 joined_at) VALUES ($1, $2, $3, 'active', $4, $4)",
            )
            .bind($chat)
            .bind(id)
            .bind(&role)
            .bind(now)
            .execute($pool)
            .await
            .unwrap();
            ids.push(id);
        }
        ids
    }};
}

pub async fn close_poll(
    base: &str,
    account: &Account,
    poll: impl std::fmt::Display,
) -> (StatusCode, Value) {
    call(
        Method::POST,
        format!("{base}/api/polls/{poll}/close"),
        &account.token,
        None,
    )
    .await
}

pub async fn retract(
    base: &str,
    account: &Account,
    poll: impl std::fmt::Display,
) -> (StatusCode, Value) {
    call(
        Method::DELETE,
        format!("{base}/api/polls/{poll}/votes"),
        &account.token,
        None,
    )
    .await
}

pub async fn get_poll(
    base: &str,
    account: &Account,
    poll: impl std::fmt::Display,
) -> (StatusCode, Value) {
    call(
        Method::GET,
        format!("{base}/api/polls/{poll}"),
        &account.token,
        None,
    )
    .await
}

pub async fn history(base: &str, account: &Account, chat: Uuid) -> Vec<Value> {
    let (status, body) = call(
        Method::GET,
        format!("{base}/api/chats/{chat}/messages"),
        &account.token,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    body.as_array().unwrap().clone()
}
