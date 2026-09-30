//! HTTP / WebSocket helpers for the TG-201 administration tests.

#![allow(dead_code)]

use std::sync::Arc;

use chat_room::{build_app, config::AppConfig, state::AppState};
use futures_util::{SinkExt, StreamExt};
use reqwest::{Client, Method, StatusCode};
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tokio_tungstenite::{connect_async, tungstenite::Message};

pub mod scenarios;

#[path = "../integration/postgres_database.rs"]
mod postgres_database;
#[path = "../service_skip/mod.rs"]
mod service_skip;

pub type Socket =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

pub struct Server {
    pub base: String,
    pub client: Client,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}

pub struct Account {
    pub id: String,
    pub token: String,
}

pub async fn serve(state: Arc<AppState>) -> Server {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        axum::serve(listener, build_app(state)).await.unwrap();
    });
    Server {
        base: format!("http://{address}"),
        client: Client::new(),
        task,
    }
}

impl Server {
    pub async fn register(&self, username: &str) -> Account {
        let response = self
            .client
            .post(format!("{}/api/users/register", self.base))
            .json(&json!({ "username": username, "password": "test-password" }))
            .send()
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            StatusCode::CREATED,
            "register {username}"
        );
        let session: Value = response.json().await.unwrap();
        Account {
            id: session["user"]["id"].as_str().unwrap().to_string(),
            token: session["token"].as_str().unwrap().to_string(),
        }
    }

    pub async fn call(
        &self,
        method: Method,
        path: &str,
        token: &str,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        let mut request = self
            .client
            .request(method, format!("{}{path}", self.base))
            .bearer_auth(token);
        if let Some(body) = body {
            request = request.json(&body);
        }
        let response = request.send().await.unwrap();
        let status = response.status();
        (status, response.json().await.unwrap_or(Value::Null))
    }

    pub async fn get(&self, path: &str, token: &str) -> (StatusCode, Value) {
        self.call(Method::GET, path, token, None).await
    }

    pub async fn put(&self, path: &str, token: &str, body: Value) -> (StatusCode, Value) {
        self.call(Method::PUT, path, token, Some(body)).await
    }

    pub async fn create_group(&self, owner: &Account, title: &str) -> String {
        let (status, chat) = self
            .call(
                Method::POST,
                "/api/chats",
                &owner.token,
                Some(json!({ "title": title, "join_policy": "open" })),
            )
            .await;
        assert_eq!(status, StatusCode::CREATED);
        chat["id"].as_str().unwrap().to_string()
    }

    pub async fn join(&self, chat_id: &str, account: &Account) {
        let (status, _) = self
            .call(
                Method::POST,
                &format!("/api/chats/{chat_id}/join-requests"),
                &account.token,
                Some(json!({ "password": null })),
            )
            .await;
        assert_eq!(status, StatusCode::OK, "join {chat_id}");
    }

    pub async fn member_count(&self, chat_id: &str, token: &str) -> i64 {
        let (status, chat) = self.get(&format!("/api/chats/{chat_id}"), token).await;
        assert_eq!(status, StatusCode::OK);
        chat["member_count"].as_i64().unwrap()
    }

    pub async fn history_contains(&self, chat_id: &str, token: &str, content: &str) -> bool {
        let (status, history) = self
            .get(&format!("/api/chats/{chat_id}/messages"), token)
            .await;
        assert_eq!(status, StatusCode::OK, "history is readable");
        history
            .as_array()
            .unwrap()
            .iter()
            .any(|message| message["content"] == content)
    }

    /// Open the chat socket and wait for `auth_ok`.
    pub async fn socket(&self, chat_id: &str, account: &Account) -> Socket {
        let url = format!("{}/ws/{chat_id}", self.base.replacen("http://", "ws://", 1));
        let (mut socket, _) = connect_async(url).await.unwrap();
        socket
            .send(Message::Text(
                json!({ "type": "join", "token": account.token }).to_string(),
            ))
            .await
            .unwrap();
        let reply = next_frame(&mut socket, "auth_ok").await;
        assert_eq!(reply["type"], "auth_ok", "{reply}");
        socket
    }
}

/// The next frame of `expected` type (or an `auth_fail`), skipping everything else.
pub async fn next_frame(socket: &mut Socket, expected: &str) -> Value {
    loop {
        let frame = tokio::time::timeout(std::time::Duration::from_secs(10), socket.next())
            .await
            .expect("frame within 10 s")
            .unwrap()
            .unwrap();
        let Message::Text(text) = frame else { continue };
        let value: Value = serde_json::from_str(&text).unwrap();
        if value["type"] == expected || value["type"] == "auth_fail" {
            return value;
        }
    }
}

pub async fn send_text(socket: &mut Socket, content: &str) {
    socket
        .send(Message::Text(
            json!({ "type": "message", "content": content }).to_string(),
        ))
        .await
        .unwrap();
}

/// Run `scenario` against a fresh PostgreSQL scratch database, or skip visibly.
pub async fn with_postgres<F, Fut>(test_name: &str, scenario: F)
where
    F: FnOnce(Arc<AppState>) -> Fut,
    Fut: std::future::Future<Output = ()>,
{
    let Some((admin_url, admin_pool)) = postgres_database::connect_postgres_admin(test_name).await
    else {
        return;
    };
    let (db_name, test_url) =
        postgres_database::create_scratch_database(&admin_pool, &admin_url).await;
    let state = Arc::new(
        AppState::open_postgres(&test_url, &AppConfig::default())
            .await
            .expect("open scratch PostgreSQL database"),
    );
    scenario(state.clone()).await;
    state.postgres_pool().unwrap().close().await;
    drop(state);
    postgres_database::drop_scratch_database(&admin_pool, &db_name).await;
}
