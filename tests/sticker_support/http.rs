//! A scratch server and the HTTP calls the sticker tests repeat.

use std::sync::Arc;

use chat_room::{build_app, config::AppConfig, state::AppState};
use reqwest::{multipart, Client, StatusCode};
use serde_json::{json, Value};
use tokio::net::TcpListener;

pub struct Server {
    pub base: String,
    pub state: Arc<AppState>,
    pub client: Client,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}

pub async fn start(config: AppConfig) -> Server {
    let state = Arc::new(AppState::new_with_config(&config).await.unwrap());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn({
        let state = state.clone();
        async move { axum::serve(listener, build_app(state)).await.unwrap() }
    });
    Server {
        base,
        state,
        client: Client::new(),
        task,
    }
}

impl Server {
    /// Register (or log in) an account and return its session token.
    pub async fn token(&self, username: &str) -> String {
        let credentials = json!({ "username": username, "password": "test-password" });
        let mut response = self
            .client
            .post(self.url("/api/users/register"))
            .json(&credentials)
            .send()
            .await
            .unwrap();
        if response.status() == StatusCode::CONFLICT {
            response = self
                .client
                .post(self.url("/api/users/login"))
                .json(&credentials)
                .send()
                .await
                .unwrap();
        }
        let body: Value = response.error_for_status().unwrap().json().await.unwrap();
        body["token"].as_str().unwrap().to_string()
    }
    /// An owner with one set holding a TGS sticker; returns (owner token, sticker JSON).
    pub async fn seeded_sticker(&self, owner_name: &str, short_name: &str) -> (String, Value) {
        let owner = self.token(owner_name).await;
        self.create_set(&owner, short_name, "regular").await;
        let (status, sticker) = self
            .upload(&owner, short_name, super::valid_tgs(), "🦀")
            .await;
        assert_eq!(status, StatusCode::CREATED);
        (owner, sticker)
    }
    pub fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base)
    }

    pub async fn user_id(&self, token: &str) -> String {
        let me: Value = self
            .client
            .get(self.url("/api/users/me"))
            .bearer_auth(token)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        me["id"].as_str().unwrap().to_string()
    }

    pub async fn create_set(&self, token: &str, short_name: &str, set_type: &str) -> Value {
        let response = self
            .client
            .post(self.url("/api/sticker-sets"))
            .bearer_auth(token)
            .json(
                &json!({ "short_name": short_name, "title": "Fixture set", "set_type": set_type }),
            )
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
        response.json().await.unwrap()
    }

    /// Upload one file; returns the status and the JSON body.
    pub async fn upload(
        &self,
        token: &str,
        short_name: &str,
        bytes: Vec<u8>,
        emoji: &str,
    ) -> (StatusCode, Value) {
        let part = multipart::Part::bytes(bytes)
            .file_name("sticker.bin")
            .mime_str("application/octet-stream")
            .unwrap();
        let form = multipart::Form::new()
            .part("file", part)
            .text("emoji", emoji.to_string());
        let response = self
            .client
            .post(self.url(&format!("/api/sticker-sets/{short_name}/stickers")))
            .bearer_auth(token)
            .multipart(form)
            .send()
            .await
            .unwrap();
        let status = response.status();
        (status, response.json().await.unwrap_or(Value::Null))
    }

    pub async fn create_chat(&self, token: &str, name: &str) -> String {
        let chat: Value = self
            .client
            .post(self.url("/api/chats"))
            .bearer_auth(token)
            .json(&json!({ "name": name, "password": "", "join_policy": "open" }))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        chat["id"].as_str().unwrap().to_string()
    }

    pub async fn send_sticker(
        &self,
        token: &str,
        room_id: &str,
        body: Value,
    ) -> (StatusCode, Value) {
        let response = self
            .client
            .post(self.url(&format!("/api/chats/{room_id}/sticker-messages")))
            .bearer_auth(token)
            .json(&body)
            .send()
            .await
            .unwrap();
        let status = response.status();
        (status, response.json().await.unwrap_or(Value::Null))
    }

    pub async fn history(&self, token: &str, room_id: &str) -> (StatusCode, Value) {
        let response = self
            .client
            .get(self.url(&format!("/api/chats/{room_id}/messages")))
            .bearer_auth(token)
            .send()
            .await
            .unwrap();
        let status = response.status();
        (status, response.json().await.unwrap_or(Value::Null))
    }

    /// GET a relative URL without credentials (capability URLs need none).
    pub async fn fetch(&self, path: &str) -> (StatusCode, Vec<u8>) {
        let response = self.client.get(self.url(path)).send().await.unwrap();
        let status = response.status();
        (status, response.bytes().await.unwrap().to_vec())
    }

    /// Befriend two accounts and open their one-to-one chat; returns its room id.
    pub async fn direct_chat(&self, first: &str, second: &str) -> String {
        let second_id = self.user_id(second).await;
        let first_id = self.user_id(first).await;
        self.client
            .post(self.url("/api/friend-requests"))
            .bearer_auth(first)
            .json(&json!({ "user_id": second_id }))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap();
        self.client
            .patch(self.url(&format!("/api/friend-requests/{first_id}")))
            .bearer_auth(second)
            .json(&json!({ "action": "accept" }))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap();
        let direct: Value = self
            .client
            .post(self.url("/api/direct-chats"))
            .bearer_auth(first)
            .json(&json!({ "user_id": second_id }))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap()
            .json()
            .await
            .unwrap();
        direct["room_id"].as_str().unwrap().to_string()
    }
}
