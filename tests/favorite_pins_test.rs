use std::sync::Arc;

use chat_room::{build_app, state::AppState};
use reqwest::{Client, StatusCode};
use tokio::net::TcpListener;

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
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move { axum::serve(listener, build_app(state)).await.unwrap() });
    TestServer {
        base: format!("http://{address}"),
        task,
    }
}

async fn register(client: &Client, base: &str, username: &str) -> String {
    let session: serde_json::Value = client
        .post(format!("{base}/api/users/register"))
        .json(&serde_json::json!({ "username": username, "password": "test-password" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    session["token"].as_str().unwrap().to_owned()
}

#[tokio::test]
async fn pinned_favorite_is_a_group_editable_document_until_unpinned() {
    let server = start_server().await;
    let client = Client::new();
    let owner = register(&client, &server.base, "pin-owner").await;
    let member = register(&client, &server.base, "pin-member").await;

    let room: serde_json::Value = client
        .post(format!("{}/api/rooms", server.base))
        .bearer_auth(&owner)
        .json(&serde_json::json!({ "name": "pin-room", "join_policy": "open" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let room_id = room["id"].as_str().unwrap();
    assert_eq!(
        client
            .post(format!("{}/api/rooms/{room_id}/join-requests", server.base))
            .bearer_auth(&member)
            .json(&serde_json::json!({}))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );

    let favorite: serde_json::Value = client
        .post(format!("{}/api/favorites", server.base))
        .bearer_auth(&owner)
        .json(&serde_json::json!({ "title": "协作文档", "content": "第一版" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let favorite_id = favorite["id"].as_str().unwrap();
    let forwarded: Vec<serde_json::Value> = client
        .post(format!(
            "{}/api/favorites/{favorite_id}/forward",
            server.base
        ))
        .bearer_auth(&owner)
        .json(&serde_json::json!({ "target_room_ids": [room_id] }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let message_id = forwarded[0]["forwarded_message_id"].as_str().unwrap();

    let pin: serde_json::Value = client
        .post(format!(
            "{}/api/rooms/{room_id}/pins/{message_id}",
            server.base
        ))
        .bearer_auth(&owner)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(pin["message"]["favorite_id"], favorite_id);

    let member_favorites: Vec<serde_json::Value> = client
        .get(format!("{}/api/favorites", server.base))
        .bearer_auth(&member)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(member_favorites[0]["id"], favorite_id);
    assert_eq!(member_favorites[0]["access"], "editor");

    assert_eq!(
        client
            .post(format!(
                "{}/api/rooms/{room_id}/pins/{message_id}",
                server.base
            ))
            .bearer_auth(&member)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    let edited: serde_json::Value = client
        .put(format!("{}/api/favorites/{favorite_id}", server.base))
        .bearer_auth(&member)
        .json(&serde_json::json!({ "version": 1, "title": "协作文档", "content": "群成员编辑" }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(edited["version"], 2);

    let messages: Vec<serde_json::Value> = client
        .get(format!("{}/api/rooms/{room_id}/messages", server.base))
        .bearer_auth(&member)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(messages[0]["content"], "群成员编辑");

    assert_eq!(
        client
            .delete(format!(
                "{}/api/rooms/{room_id}/pins/{message_id}",
                server.base
            ))
            .bearer_auth(&owner)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::NO_CONTENT
    );
    let after_unpin: Vec<serde_json::Value> = client
        .get(format!("{}/api/favorites", server.base))
        .bearer_auth(&member)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(after_unpin.is_empty());

    let owner_user: serde_json::Value = client
        .get(format!("{}/api/users/me", server.base))
        .bearer_auth(&owner)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let member_user: serde_json::Value = client
        .get(format!("{}/api/users/me", server.base))
        .bearer_auth(&member)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    client
        .post(format!("{}/api/friend-requests", server.base))
        .bearer_auth(&owner)
        .json(&serde_json::json!({ "user_id": member_user["id"] }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    client
        .patch(format!(
            "{}/api/friend-requests/{}",
            server.base,
            owner_user["id"].as_str().unwrap()
        ))
        .bearer_auth(&member)
        .json(&serde_json::json!({ "action": "accept" }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    let direct: serde_json::Value = client
        .post(format!("{}/api/direct-chats", server.base))
        .bearer_auth(&owner)
        .json(&serde_json::json!({ "user_id": member_user["id"] }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let direct_room_id = direct["room_id"].as_str().unwrap();
    let mut direct_message_ids = Vec::new();
    for _ in 0..2 {
        let forwarded: Vec<serde_json::Value> = client
            .post(format!(
                "{}/api/favorites/{favorite_id}/forward",
                server.base
            ))
            .bearer_auth(&owner)
            .json(&serde_json::json!({ "target_room_ids": [direct_room_id] }))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap()
            .json()
            .await
            .unwrap();
        direct_message_ids.push(
            forwarded[0]["forwarded_message_id"]
                .as_str()
                .unwrap()
                .to_owned(),
        );
    }
    for (token, message_id) in [
        (&owner, &direct_message_ids[0]),
        (&member, &direct_message_ids[1]),
    ] {
        assert_eq!(
            client
                .post(format!(
                    "{}/api/rooms/{direct_room_id}/pins/{message_id}",
                    server.base
                ))
                .bearer_auth(token)
                .send()
                .await
                .unwrap()
                .status(),
            StatusCode::CREATED
        );
    }
    let direct_pins: Vec<serde_json::Value> = client
        .get(format!("{}/api/rooms/{direct_room_id}/pins", server.base))
        .bearer_auth(&member)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(direct_pins.len(), 2);
}
