use std::sync::Arc;

use chat_room::{build_app, state::AppState};
use reqwest::{Client, StatusCode};
use tokio::net::TcpListener;

async fn start_server() -> (String, Arc<AppState>, tokio::task::JoinHandle<()>) {
    let state = Arc::new(AppState::new().await.unwrap());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server_state = state.clone();
    let task = tokio::spawn(async move {
        axum::serve(listener, build_app(server_state))
            .await
            .unwrap()
    });
    (format!("http://{address}"), state, task)
}

async fn register(client: &Client, base: &str, username: &str) -> String {
    client
        .post(format!("{base}/api/users/register"))
        .json(&serde_json::json!({ "username": username, "password": "test-password" }))
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap()["token"]
        .as_str()
        .unwrap()
        .to_owned()
}

#[tokio::test]
async fn ai_threads_are_persistent_editable_and_private() {
    let (base, state, task) = start_server().await;
    let client = Client::new();
    let owner = register(&client, &base, "thread-owner").await;
    let outsider = register(&client, &base, "thread-outsider").await;

    let created = client
        .post(format!("{base}/api/ai/threads"))
        .bearer_auth(&owner)
        .json(&serde_json::json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(created.status(), StatusCode::CREATED);
    let created = created.json::<serde_json::Value>().await.unwrap();
    assert_eq!(created["title"], "新对话");
    assert_eq!(created["thinking_enabled"], false);
    assert!(created["room_id"].is_null());
    let thread_id = created["id"].as_str().unwrap();
    let thread_id = uuid::Uuid::parse_str(thread_id).unwrap();

    let updated = client
        .patch(format!("{base}/api/ai/threads/{thread_id}"))
        .bearer_auth(&owner)
        .json(&serde_json::json!({ "title": "项目回顾", "thinking_enabled": true }))
        .send()
        .await
        .unwrap();
    assert_eq!(updated.status(), StatusCode::OK);
    let updated = updated.json::<serde_json::Value>().await.unwrap();
    assert_eq!(updated["title"], "项目回顾");
    assert_eq!(updated["thinking_enabled"], true);

    let threads = client
        .get(format!("{base}/api/ai/threads"))
        .bearer_auth(&owner)
        .send()
        .await
        .unwrap()
        .json::<Vec<serde_json::Value>>()
        .await
        .unwrap();
    assert_eq!(threads.len(), 1);
    assert_eq!(threads[0]["id"], thread_id.to_string());

    let owner_token = uuid::Uuid::parse_str(&owner).unwrap();
    let owner_user = state.session_user(owner_token).await.unwrap().unwrap();
    state
        .append_ai_thread_message(owner_user.id, thread_id, "user", "第一问", None, None)
        .await
        .unwrap()
        .unwrap();
    state
        .append_ai_thread_message(
            owner_user.id,
            thread_id,
            "assistant",
            "第一答",
            None,
            Some(0),
        )
        .await
        .unwrap()
        .unwrap();
    let messages = client
        .get(format!("{base}/api/ai/threads/{thread_id}/messages"))
        .bearer_auth(&owner)
        .send()
        .await
        .unwrap()
        .json::<Vec<serde_json::Value>>()
        .await
        .unwrap();
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0]["content"], "第一问");
    assert_eq!(messages[1]["content"], "第一答");

    assert_eq!(
        client
            .get(format!("{base}/api/ai/threads/{thread_id}/messages"))
            .bearer_auth(&outsider)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::NOT_FOUND
    );
    task.abort();
}
