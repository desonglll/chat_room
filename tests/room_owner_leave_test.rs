use std::sync::Arc;

use chat_room::{build_app, state::AppState};
use reqwest::StatusCode;
use tokio::net::TcpListener;

mod support;
use support::session_token;

struct TestServer {
    base: String,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn start_server() -> TestServer {
    let state = Arc::new(AppState::new().await.unwrap());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        axum::serve(listener, build_app(state)).await.unwrap();
    });
    TestServer { base, task }
}

async fn account(base: &str, username: &str) -> (String, String) {
    let token = session_token(base, username).await;
    let user: serde_json::Value = reqwest::Client::new()
        .get(format!("{base}/api/users/me"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    (token, user["id"].as_str().unwrap().to_string())
}

async fn create_room(base: &str, token: &str, name: &str) -> String {
    reqwest::Client::new()
        .post(format!("{base}/api/rooms"))
        .bearer_auth(token)
        .json(&serde_json::json!({
            "name": name,
            "password": "",
            "join_policy": "open"
        }))
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string()
}

#[tokio::test]
async fn owner_leave_transfers_room_and_removes_the_old_conversation() {
    let server = start_server().await;
    let client = reqwest::Client::new();
    let (owner_token, _) = account(&server.base, "leaving-owner").await;
    let (member_token, member_id) = account(&server.base, "leaving-successor").await;
    let room_id = create_room(&server.base, &owner_token, "owner-leave-room").await;

    let join = client
        .post(format!("{}/api/rooms/{room_id}/join-requests", server.base))
        .bearer_auth(&member_token)
        .json(&serde_json::json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(join.status(), StatusCode::OK);

    let leave = client
        .delete(format!("{}/api/rooms/{room_id}/members/me", server.base))
        .bearer_auth(&owner_token)
        .send()
        .await
        .unwrap();
    assert_eq!(leave.status(), StatusCode::NO_CONTENT);

    let owner_conversations: Vec<serde_json::Value> = client
        .get(format!("{}/api/conversations", server.base))
        .bearer_auth(&owner_token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(owner_conversations
        .iter()
        .all(|conversation| conversation["room_id"] != room_id));

    let room: serde_json::Value = client
        .get(format!("{}/api/rooms/{room_id}", server.base))
        .bearer_auth(&member_token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(room["creator_user_id"], member_id);
    assert_eq!(room["membership_role"], "owner");
}

#[tokio::test]
async fn sole_owner_leave_is_rejected_without_removing_the_conversation() {
    let server = start_server().await;
    let client = reqwest::Client::new();
    let (owner_token, owner_id) = account(&server.base, "sole-owner").await;
    let room_id = create_room(&server.base, &owner_token, "sole-owner-room").await;

    let leave = client
        .delete(format!("{}/api/rooms/{room_id}/members/me", server.base))
        .bearer_auth(&owner_token)
        .send()
        .await
        .unwrap();
    assert_eq!(leave.status(), StatusCode::CONFLICT);

    let conversations: Vec<serde_json::Value> = client
        .get(format!("{}/api/conversations", server.base))
        .bearer_auth(&owner_token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(conversations
        .iter()
        .any(|conversation| conversation["room_id"] == room_id));

    let room: serde_json::Value = client
        .get(format!("{}/api/rooms/{room_id}", server.base))
        .bearer_auth(&owner_token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(room["creator_user_id"], owner_id);
    assert_eq!(room["membership_role"], "owner");
}
