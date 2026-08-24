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
    let task = tokio::spawn(async move {
        axum::serve(listener, build_app(state)).await.unwrap();
    });
    TestServer {
        base: format!("http://{address}"),
        task,
    }
}

async fn register(client: &Client, base: &str, username: &str) -> (String, String) {
    let value: serde_json::Value = client
        .post(format!("{base}/api/users/register"))
        .json(&serde_json::json!({ "username": username, "password": "test-password" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    (
        value["user"]["id"].as_str().unwrap().to_string(),
        value["token"].as_str().unwrap().to_string(),
    )
}

#[tokio::test]
async fn friend_remarks_are_private_and_become_the_direct_chat_title() {
    let server = start_server().await;
    let client = Client::new();
    let (alice_id, alice_token) = register(&client, &server.base, "remark-alice").await;
    let (bob_id, bob_token) = register(&client, &server.base, "remark-bob").await;
    let (outsider_id, outsider_token) = register(&client, &server.base, "remark-outsider").await;

    assert_eq!(
        client
            .post(format!("{}/api/friend-requests", server.base))
            .bearer_auth(&alice_token)
            .json(&serde_json::json!({ "user_id": bob_id }))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::CREATED
    );
    assert_eq!(
        client
            .patch(format!("{}/api/friend-requests/{alice_id}", server.base))
            .bearer_auth(&bob_token)
            .json(&serde_json::json!({ "action": "accept" }))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );

    assert_eq!(
        client
            .put(format!("{}/api/friends/{bob_id}/remark", server.base))
            .bearer_auth(&alice_token)
            .json(&serde_json::json!({ "remark": "  项目负责人  " }))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::NO_CONTENT
    );

    let alice_friends: Vec<serde_json::Value> = client
        .get(format!("{}/api/friends", server.base))
        .bearer_auth(&alice_token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let bob_friends: Vec<serde_json::Value> = client
        .get(format!("{}/api/friends", server.base))
        .bearer_auth(&bob_token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(alice_friends[0]["remark"], "项目负责人");
    assert_eq!(bob_friends[0]["remark"], "");

    let direct: serde_json::Value = client
        .post(format!("{}/api/direct-chats", server.base))
        .bearer_auth(&alice_token)
        .json(&serde_json::json!({ "user_id": bob_id }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(direct["title"], "项目负责人");

    assert_eq!(
        client
            .put(format!("{}/api/friends/{outsider_id}/remark", server.base))
            .bearer_auth(&alice_token)
            .json(&serde_json::json!({ "remark": "not allowed" }))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        client
            .put(format!("{}/api/friends/{alice_id}/remark", server.base))
            .bearer_auth(&outsider_token)
            .json(&serde_json::json!({ "remark": "not allowed" }))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::NOT_FOUND
    );

    assert_eq!(
        client
            .delete(format!("{}/api/friends/{bob_id}", server.base))
            .bearer_auth(&alice_token)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::NO_CONTENT
    );
    client
        .post(format!("{}/api/friend-requests", server.base))
        .bearer_auth(&alice_token)
        .json(&serde_json::json!({ "user_id": bob_id }))
        .send()
        .await
        .unwrap();
    client
        .patch(format!("{}/api/friend-requests/{alice_id}", server.base))
        .bearer_auth(&bob_token)
        .json(&serde_json::json!({ "action": "accept" }))
        .send()
        .await
        .unwrap();
    let friends_after_readd: Vec<serde_json::Value> = client
        .get(format!("{}/api/friends", server.base))
        .bearer_auth(&alice_token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(friends_after_readd[0]["remark"], "");
}
