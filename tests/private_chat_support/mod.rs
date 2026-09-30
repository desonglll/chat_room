//! HTTP / WebSocket helpers for `private_chat_path_test.rs`.

use std::sync::Arc;

use chat_room::{build_app, state::AppState};
use futures_util::{SinkExt, StreamExt};
use reqwest::{Client, StatusCode};
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tokio_tungstenite::{connect_async, tungstenite::Message};

pub type Socket =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

pub struct Account {
    pub id: String,
    pub token: String,
}

pub struct Server {
    pub base: String,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}

pub async fn serve(state: Arc<AppState>) -> Server {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        axum::serve(listener, build_app(state)).await.unwrap();
    });
    Server {
        base: format!("http://{address}"),
        task,
    }
}

pub async fn register(client: &Client, base: &str, username: &str, display_name: &str) -> Account {
    let response = client
        .post(format!("{base}/api/users/register"))
        .json(&json!({ "username": username, "password": "test-password" }))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let session: Value = response.json().await.unwrap();
    let account = Account {
        id: session["user"]["id"].as_str().unwrap().to_string(),
        token: session["token"].as_str().unwrap().to_string(),
    };
    let profile = client
        .patch(format!("{base}/api/users/me"))
        .bearer_auth(&account.token)
        .json(&json!({ "display_name": display_name, "avatar_emoji": "🦊" }))
        .send()
        .await
        .unwrap();
    assert_eq!(profile.status(), StatusCode::OK, "profile update");
    account
}

pub async fn befriend(client: &Client, base: &str, from: &Account, to: &Account) {
    let requested = client
        .post(format!("{base}/api/friend-requests"))
        .bearer_auth(&from.token)
        .json(&json!({ "user_id": to.id }))
        .send()
        .await
        .unwrap();
    assert_eq!(requested.status(), StatusCode::CREATED);
    let accepted = client
        .patch(format!("{base}/api/friend-requests/{}", from.id))
        .bearer_auth(&to.token)
        .json(&json!({ "action": "accept" }))
        .send()
        .await
        .unwrap();
    assert_eq!(accepted.status(), StatusCode::OK);
}

pub async fn get_json(client: &Client, url: String, token: &str) -> (StatusCode, Value) {
    let response = client.get(url).bearer_auth(token).send().await.unwrap();
    let status = response.status();
    (status, response.json().await.unwrap_or(Value::Null))
}

pub async fn next_type(socket: &mut Socket, expected: &str) -> Value {
    loop {
        let frame = socket.next().await.unwrap().unwrap();
        let Message::Text(text) = frame else { continue };
        let value: Value = serde_json::from_str(&text).unwrap();
        if value["type"] == expected || value["type"] == "auth_fail" {
            return value;
        }
    }
}

/// Open the chat socket and return the first `auth_ok` or `auth_fail` frame.
pub async fn join(base: &str, chat_id: &str, token: &str) -> (Socket, Value) {
    let url = format!("{}/ws/{chat_id}", base.replacen("http://", "ws://", 1));
    let (mut socket, _) = connect_async(url).await.unwrap();
    socket
        .send(Message::Text(
            json!({ "type": "join", "token": token }).to_string(),
        ))
        .await
        .unwrap();
    let reply = next_type(&mut socket, "auth_ok").await;
    (socket, reply)
}

/// Send one message and hand back the still-open socket. The caller keeps it open until the
/// scenario ends: dropping a socket mid-request can cancel a query on the test database's only
/// in-memory SQLite connection, which the pool then replaces with a fresh, empty database.
pub async fn send_message(base: &str, chat_id: &str, token: &str, content: &str) -> Socket {
    let (mut socket, reply) = join(base, chat_id, token).await;
    assert_eq!(
        reply["type"], "auth_ok",
        "participant joins {chat_id}: {reply}"
    );
    socket
        .send(Message::Text(
            json!({ "type": "message", "content": content }).to_string(),
        ))
        .await
        .unwrap();
    assert_eq!(
        next_type(&mut socket, "broadcast").await["content"],
        content
    );
    socket
}
