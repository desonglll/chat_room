use std::sync::Arc;

use chat_room::{build_app, state::AppState};
use reqwest::{Client, StatusCode};
use tokio::net::TcpListener;
use uuid::Uuid;

async fn start_server() -> (String, tokio::task::JoinHandle<()>) {
    let state = Arc::new(AppState::new().await.unwrap());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move { axum::serve(listener, build_app(state)).await.unwrap() });
    (format!("http://{address}"), task)
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
async fn conversation_ai_checks_identity_and_membership_before_provider_availability() {
    let (base, task) = start_server().await;
    let client = Client::new();
    let owner = register(&client, &base, "ai-owner").await;
    let outsider = register(&client, &base, "ai-outsider").await;
    let room: serde_json::Value = client
        .post(format!("{base}/api/rooms"))
        .bearer_auth(&owner)
        .json(&serde_json::json!({ "name": "AI room", "password": "room-secret", "join_policy": "open" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let endpoint = format!(
        "{base}/api/ai/conversations/{}/query",
        room["id"].as_str().unwrap()
    );
    let payload = serde_json::json!({ "question": "总结一下", "history": [] });

    assert_eq!(
        client
            .post(&endpoint)
            .json(&payload)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        client
            .post(&endpoint)
            .bearer_auth(&outsider)
            .json(&payload)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        client
            .post(&endpoint)
            .bearer_auth(&owner)
            .json(&payload)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        client
            .post(&endpoint)
            .bearer_auth(&owner)
            .header("x-room-password", "room-secret")
            .json(&payload)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    task.abort();
}

#[tokio::test]
async fn conversation_ai_stream_requires_identity_before_room_lookup() {
    let (base, task) = start_server().await;
    let response = Client::new()
        .post(format!(
            "{base}/api/ai/conversations/{}/query/stream",
            Uuid::new_v4()
        ))
        .json(&serde_json::json!({ "question": "总结一下", "history": [] }))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    task.abort();
}
