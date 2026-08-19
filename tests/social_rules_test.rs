use std::sync::Arc;

use chat_room::{build_app, state::AppState};
use reqwest::{Client, StatusCode};
use tokio::net::TcpListener;

struct Account {
    id: String,
    token: String,
}

async fn register(client: &Client, base: &str, username: &str) -> Account {
    let value = client
        .post(format!("{base}/api/users/register"))
        .json(&serde_json::json!({ "username": username, "password": "test-password" }))
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();
    Account {
        id: value["user"]["id"].as_str().unwrap().to_string(),
        token: value["token"].as_str().unwrap().to_string(),
    }
}

#[tokio::test]
async fn requests_are_cancelable_cross_requests_accept_and_direct_room_is_constrained() {
    let state = Arc::new(AppState::new().await.unwrap());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        axum::serve(listener, build_app(state)).await.unwrap();
    });
    let client = Client::new();
    let alice = register(&client, &base, "rules-alice").await;
    let bob = register(&client, &base, "rules-bob").await;
    let charlie = register(&client, &base, "rules-charlie").await;

    assert_eq!(
        client
            .post(format!("{base}/api/friend-requests"))
            .bearer_auth(&alice.token)
            .json(&serde_json::json!({ "user_id": charlie.id }))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::CREATED
    );
    assert_eq!(
        client
            .delete(format!("{base}/api/friend-requests/{}", charlie.id))
            .bearer_auth(&alice.token)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::NO_CONTENT
    );
    let incoming = client
        .get(format!("{base}/api/friend-requests?direction=incoming"))
        .bearer_auth(&charlie.token)
        .send()
        .await
        .unwrap()
        .json::<Vec<serde_json::Value>>()
        .await
        .unwrap();
    assert!(incoming.is_empty());

    client
        .post(format!("{base}/api/friend-requests"))
        .bearer_auth(&alice.token)
        .json(&serde_json::json!({ "user_id": bob.id }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        client
            .post(format!("{base}/api/friend-requests"))
            .bearer_auth(&bob.token)
            .json(&serde_json::json!({ "user_id": alice.id }))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );

    let alice_start = client
        .post(format!("{base}/api/direct-chats"))
        .bearer_auth(&alice.token)
        .json(&serde_json::json!({ "user_id": bob.id }))
        .send();
    let bob_start = client
        .post(format!("{base}/api/direct-chats"))
        .bearer_auth(&bob.token)
        .json(&serde_json::json!({ "user_id": alice.id }))
        .send();
    let (alice_chat, bob_chat) = tokio::join!(alice_start, bob_start);
    let alice_chat = alice_chat.unwrap();
    let bob_chat = bob_chat.unwrap();
    assert_eq!(alice_chat.status(), StatusCode::OK);
    assert_eq!(bob_chat.status(), StatusCode::OK);
    let alice_chat: serde_json::Value = alice_chat.json().await.unwrap();
    let bob_chat: serde_json::Value = bob_chat.json().await.unwrap();
    assert_eq!(alice_chat["room_id"], bob_chat["room_id"]);
    let room_id = alice_chat["room_id"].as_str().unwrap();

    assert_eq!(
        client
            .patch(format!("{base}/api/rooms/{room_id}"))
            .bearer_auth(&alice.token)
            .json(&serde_json::json!({ "name": "not-a-group" }))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        client
            .patch(format!("{base}/api/rooms/{room_id}/members/me"))
            .bearer_auth(&alice.token)
            .json(&serde_json::json!({ "nickname": "hidden" }))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        client
            .delete(format!("{base}/api/rooms/{room_id}/members/me"))
            .bearer_auth(&bob.token)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        client
            .post(format!("{base}/api/rooms/{room_id}/invitations"))
            .bearer_auth(&alice.token)
            .json(&serde_json::json!({ "username": "someone" }))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::NOT_FOUND
    );
    task.abort();
}

#[tokio::test]
async fn deleting_an_account_closes_its_direct_conversations() {
    let state = Arc::new(AppState::new().await.unwrap());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        axum::serve(listener, build_app(state)).await.unwrap();
    });
    let client = Client::new();
    let alice = register(&client, &base, "delete-direct-alice").await;
    let bob = register(&client, &base, "delete-direct-bob").await;

    client
        .post(format!("{base}/api/friend-requests"))
        .bearer_auth(&alice.token)
        .json(&serde_json::json!({ "user_id": bob.id }))
        .send()
        .await
        .unwrap();
    client
        .patch(format!("{base}/api/friend-requests/{}", alice.id))
        .bearer_auth(&bob.token)
        .json(&serde_json::json!({ "action": "accept" }))
        .send()
        .await
        .unwrap();
    let conversation = client
        .post(format!("{base}/api/direct-chats"))
        .bearer_auth(&alice.token)
        .json(&serde_json::json!({ "user_id": bob.id }))
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();
    let room_id = conversation["room_id"].as_str().unwrap();

    assert_eq!(
        client
            .delete(format!("{base}/api/users/me"))
            .bearer_auth(&alice.token)
            .json(&serde_json::json!({ "current_password": "test-password" }))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::NO_CONTENT
    );
    let conversations = client
        .get(format!("{base}/api/conversations"))
        .bearer_auth(&bob.token)
        .send()
        .await
        .unwrap()
        .json::<Vec<serde_json::Value>>()
        .await
        .unwrap();
    assert!(conversations.is_empty());
    let rooms = client
        .get(format!("{base}/api/rooms"))
        .bearer_auth(&bob.token)
        .send()
        .await
        .unwrap()
        .json::<Vec<serde_json::Value>>()
        .await
        .unwrap();
    assert!(rooms.iter().all(|room| room["id"] != room_id));
    assert_eq!(
        client
            .get(format!("{base}/api/rooms/{room_id}"))
            .bearer_auth(&bob.token)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::NOT_FOUND
    );
    task.abort();
}
