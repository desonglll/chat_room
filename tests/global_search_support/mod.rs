use std::sync::Arc;

use chat_room::{build_app, state::AppState};
use chrono::{DateTime, Utc};
use reqwest::Client;
use tokio::net::TcpListener;
use uuid::Uuid;

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

pub struct Account {
    pub id: Uuid,
    pub token: String,
}

pub async fn start_server() -> TestServer {
    let state = Arc::new(AppState::new().await.unwrap());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn({
        let state = state.clone();
        async move { axum::serve(listener, build_app(state)).await.unwrap() }
    });
    TestServer {
        base: format!("http://{address}"),
        state,
        task,
    }
}

pub async fn register(client: &Client, base: &str, username: &str) -> Account {
    let session: serde_json::Value = client
        .post(format!("{base}/api/users/register"))
        .json(&serde_json::json!({ "username": username, "password": "test-password" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    Account {
        id: Uuid::parse_str(session["user"]["id"].as_str().unwrap()).unwrap(),
        token: session["token"].as_str().unwrap().to_owned(),
    }
}

pub async fn create_room(client: &Client, base: &str, account: &Account, name: &str) -> Uuid {
    let room: serde_json::Value = client
        .post(format!("{base}/api/rooms"))
        .bearer_auth(&account.token)
        .json(&serde_json::json!({ "name": name, "join_policy": "open" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    Uuid::parse_str(room["id"].as_str().unwrap()).unwrap()
}

pub async fn create_direct_chat(
    client: &Client,
    base: &str,
    requester: &Account,
    recipient: &Account,
) -> Uuid {
    let requested = client
        .post(format!("{base}/api/friend-requests"))
        .bearer_auth(&requester.token)
        .json(&serde_json::json!({ "user_id": recipient.id }))
        .send()
        .await
        .unwrap();
    assert!(requested.status().is_success());
    let accepted = client
        .patch(format!("{base}/api/friend-requests/{}", requester.id))
        .bearer_auth(&recipient.token)
        .json(&serde_json::json!({ "action": "accept" }))
        .send()
        .await
        .unwrap();
    assert!(accepted.status().is_success());
    let direct: serde_json::Value = client
        .post(format!("{base}/api/direct-chats"))
        .bearer_auth(&requester.token)
        .json(&serde_json::json!({ "user_id": recipient.id }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    Uuid::parse_str(direct["room_id"].as_str().unwrap()).unwrap()
}

pub async fn insert_message(
    state: &AppState,
    id: Uuid,
    room_id: Uuid,
    sender: &Account,
    content: &str,
    attachment_id: Option<Uuid>,
    created_at: DateTime<Utc>,
) {
    sqlx::query(
        "INSERT INTO messages (id, room_id, sender_id, sender, content, attachment_id, created_at) \
         VALUES (?, ?, ?, 'search-sender', ?, ?, ?)",
    )
    .bind(id)
    .bind(room_id)
    .bind(sender.id)
    .bind(content)
    .bind(attachment_id)
    .bind(created_at)
    .execute(state.pool())
    .await
    .unwrap();
}

pub async fn search(client: &Client, base: &str, token: &str, query: &str) -> serde_json::Value {
    client
        .get(format!("{base}/api/messages/search?{query}"))
        .bearer_auth(token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap()
}

pub async fn search_after(
    client: &Client,
    base: &str,
    token: &str,
    cursor: &str,
) -> serde_json::Value {
    client
        .get(format!("{base}/api/messages/search"))
        .bearer_auth(token)
        .query(&[("q", "needle"), ("limit", "2"), ("cursor", cursor)])
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap()
}
