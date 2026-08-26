use std::sync::Arc;

use chat_room::{build_app, state::AppState};
use reqwest::{Client, StatusCode};
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

pub async fn register(client: &Client, base: &str, username: &str) -> (String, Uuid) {
    let session: serde_json::Value = client
        .post(format!("{base}/api/users/register"))
        .json(&serde_json::json!({
            "username": username,
            "password": "test-password"
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    (
        session["token"].as_str().unwrap().to_owned(),
        Uuid::parse_str(session["user"]["id"].as_str().unwrap()).unwrap(),
    )
}

pub async fn create_room(client: &Client, base: &str, token: &str, name: &str) -> Uuid {
    let room: serde_json::Value = client
        .post(format!("{base}/api/rooms"))
        .bearer_auth(token)
        .json(&serde_json::json!({ "name": name, "password": "", "join_policy": "open" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    Uuid::parse_str(room["id"].as_str().unwrap()).unwrap()
}

pub async fn make_friends(
    client: &Client,
    base: &str,
    requester_token: &str,
    requester_id: Uuid,
    recipient_token: &str,
    recipient_id: Uuid,
) {
    assert_eq!(
        client
            .post(format!("{base}/api/friend-requests"))
            .bearer_auth(requester_token)
            .json(&serde_json::json!({ "user_id": recipient_id }))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::CREATED
    );
    assert_eq!(
        client
            .patch(format!("{base}/api/friend-requests/{requester_id}"))
            .bearer_auth(recipient_token)
            .json(&serde_json::json!({ "action": "accept" }))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
}
