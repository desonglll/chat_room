//! Shared live-server helpers for the TG-505 privacy tests.
//!
//! The server runs on a caller-supplied `AppState`, so one scenario body can be executed
//! against SQLite and against a PostgreSQL scratch database alike.

#![allow(dead_code)]

use std::sync::Arc;
use std::time::Duration;

use chat_room::{build_app, state::AppState};
use futures_util::{SinkExt, StreamExt};
use reqwest::{multipart, Client, StatusCode};
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use uuid::Uuid;

#[path = "../migration_support/mod.rs"]
pub mod migration_support;

pub const PNG: &[u8] = &[
    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 6, 0,
    0, 0, 31, 21, 196, 137, 0, 0, 0, 13, 73, 68, 65, 84, 8, 215, 99, 248, 207, 192, 240, 31, 0, 5,
    0, 1, 255, 137, 153, 61, 29, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
];

pub const KEYS: [&str; 5] = [
    "last_seen",
    "profile_photo",
    "forwards",
    "group_invites",
    "voice_messages",
];
pub const TIERS: [&str; 3] = ["everybody", "contacts", "nobody"];

pub type Socket =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

pub struct TestServer {
    pub base: String,
    pub state: Arc<AppState>,
    pub client: Client,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

#[derive(Clone, Debug)]
pub struct Account {
    pub name: String,
    pub token: String,
    pub id: Uuid,
}

pub async fn start_server_on(state: AppState) -> TestServer {
    let state = Arc::new(state);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let app = build_app(state.clone());
    let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    TestServer {
        base: format!("http://{address}"),
        state,
        client: Client::new(),
        task,
    }
}

/// A server on the default in-memory `AppState::new()`.
///
/// TG-111 made that database survive pooled-connection churn (closing a WebSocket aborts
/// its forwarder mid-query), so these suites no longer need a file-backed workaround.
pub async fn start_server() -> TestServer {
    start_server_on(AppState::new().await.unwrap()).await
}

impl TestServer {
    pub fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base)
    }

    pub async fn account(&self, name: &str) -> Account {
        let response = self
            .client
            .post(self.url("/api/users/register"))
            .json(&json!({ "username": name, "password": "test-password" }))
            .send()
            .await
            .unwrap();
        assert!(response.status().is_success(), "register {name}");
        let body: Value = response.json().await.unwrap();
        let token = body["token"].as_str().unwrap().to_string();
        let id = self
            .state
            .session_user(Uuid::parse_str(&token).unwrap())
            .await
            .unwrap()
            .unwrap()
            .id;
        Account {
            name: name.to_string(),
            token,
            id,
        }
    }

    pub async fn befriend(&self, left: &Account, right: &Account) {
        let requested = self
            .client
            .post(self.url("/api/friend-requests"))
            .bearer_auth(&left.token)
            .json(&json!({ "user_id": right.id }))
            .send()
            .await
            .unwrap();
        assert!(requested.status().is_success());
        let accepted = self
            .client
            .patch(self.url(&format!("/api/friend-requests/{}", left.id)))
            .bearer_auth(&right.token)
            .json(&json!({ "action": "accept" }))
            .send()
            .await
            .unwrap();
        assert_eq!(accepted.status(), StatusCode::OK);
    }

    pub async fn block(&self, blocker: &Account, blocked: &Account) {
        let response = self
            .client
            .put(self.url(&format!("/api/blocks/{}", blocked.id)))
            .bearer_auth(&blocker.token)
            .send()
            .await
            .unwrap();
        assert!(response.status().is_success());
    }

    pub async fn put_rule(
        &self,
        owner: &Account,
        key: &str,
        tier: &str,
        allow: &[&Account],
        deny: &[&Account],
    ) -> (StatusCode, Value) {
        let response = self
            .client
            .put(self.url(&format!("/api/users/me/privacy/{key}")))
            .bearer_auth(&owner.token)
            .json(&json!({
                "tier": tier,
                "allow_user_ids": allow.iter().map(|a| a.id).collect::<Vec<_>>(),
                "deny_user_ids": deny.iter().map(|a| a.id).collect::<Vec<_>>(),
            }))
            .send()
            .await
            .unwrap();
        let status = response.status();
        let text = response.text().await.unwrap();
        (status, serde_json::from_str(&text).unwrap_or(Value::Null))
    }

    pub async fn create_group(&self, owner: &Account, title: &str) -> Uuid {
        let body: Value = self
            .client
            .post(self.url("/api/chats"))
            .bearer_auth(&owner.token)
            .json(&json!({ "title": title, "password": "", "join_policy": "open" }))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        Uuid::parse_str(body["id"].as_str().unwrap()).unwrap()
    }

    pub async fn upload_avatar(&self, owner: &Account) -> String {
        let form = multipart::Form::new().part(
            "file",
            multipart::Part::bytes(PNG.to_vec())
                .file_name("avatar.png")
                .mime_str("image/png")
                .unwrap(),
        );
        let user: Value = self
            .client
            .post(self.url("/api/users/me/avatar"))
            .bearer_auth(&owner.token)
            .multipart(form)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        user["avatar_emoji"].as_str().unwrap().to_string()
    }

    pub async fn open_chat(&self, room_id: Uuid, account: &Account) -> (Socket, Value) {
        let url = format!("{}/ws/{room_id}", self.base.replacen("http://", "ws://", 1));
        let (mut socket, _) = connect_async(url).await.unwrap();
        socket
            .send(Message::Text(
                json!({ "type": "join", "token": account.token }).to_string(),
            ))
            .await
            .unwrap();
        let auth = next_json(&mut socket).await;
        assert_eq!(auth["type"], "auth_ok", "{auth}");
        (socket, auth)
    }

    /// Close a socket and wait until the server has processed the disconnect.
    pub async fn close_and_settle(&self, room_id: Uuid, account: &Account, mut socket: Socket) {
        socket.close(None).await.ok();
        for _ in 0..200 {
            let connected = self.state.connected_members(room_id).await;
            if !connected.iter().any(|member| member.user_id == account.id) {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("{} never disconnected", account.name);
    }

    /// Join `room_id` once through the WebSocket (open chats auto-join) and leave.
    pub async fn join(&self, room_id: Uuid, account: &Account) {
        let (socket, _) = self.open_chat(room_id, account).await;
        self.close_and_settle(room_id, account, socket).await;
    }

    /// Send a text message and return its id from the echoed broadcast.
    pub async fn send_message(&self, room_id: Uuid, account: &Account, content: &str) -> Uuid {
        let (mut socket, _) = self.open_chat(room_id, account).await;
        socket
            .send(Message::Text(
                json!({ "type": "message", "content": content }).to_string(),
            ))
            .await
            .unwrap();
        loop {
            let frame = next_json(&mut socket).await;
            if frame["type"] == "broadcast" && frame["content"] == content {
                let id = Uuid::parse_str(frame["message_id"].as_str().unwrap()).unwrap();
                self.close_and_settle(room_id, account, socket).await;
                return id;
            }
        }
    }
}

pub async fn next_json(socket: &mut Socket) -> Value {
    loop {
        let frame = tokio::time::timeout(Duration::from_secs(5), socket.next())
            .await
            .expect("timed out waiting for WebSocket frame")
            .expect("WebSocket ended")
            .expect("WebSocket error");
        let Message::Text(text) = frame else { continue };
        let value: Value = serde_json::from_str(&text).unwrap();
        if value["type"] != "history_complete" {
            return value;
        }
    }
}

/// The status `auth_ok` reports for `user_id`.
pub fn status_of(auth: &Value, user_id: Uuid) -> Value {
    auth["statuses"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["user_id"] == user_id.to_string())
        .map(|entry| entry["status"].clone())
        .unwrap_or_else(|| panic!("no status for {user_id} in {auth}"))
}
