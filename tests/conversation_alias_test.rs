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

struct Account {
    id: String,
    token: String,
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

async fn register(client: &Client, base: &str, username: &str) -> Account {
    let value = client
        .post(format!("{base}/api/users/register"))
        .json(&serde_json::json!({
            "username": username,
            "password": "test-password"
        }))
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

async fn set_alias(
    client: &Client,
    base: &str,
    token: &str,
    room_id: &str,
    alias: &str,
) -> reqwest::Response {
    client
        .put(format!("{base}/api/conversations/{room_id}/alias"))
        .bearer_auth(token)
        .json(&serde_json::json!({ "alias": alias }))
        .send()
        .await
        .unwrap()
}

async fn conversations(client: &Client, base: &str, token: &str) -> Vec<serde_json::Value> {
    client
        .get(format!("{base}/api/conversations"))
        .bearer_auth(token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap()
}

fn conversation<'a>(items: &'a [serde_json::Value], room_id: &str) -> &'a serde_json::Value {
    items
        .iter()
        .find(|item| item["room_id"] == room_id)
        .expect("conversation missing")
}

#[tokio::test]
async fn aliases_are_private_per_user_for_group_and_direct_conversations() {
    let server = start_server().await;
    let client = Client::new();
    let alice = register(&client, &server.base, "alias-alice").await;
    let bob = register(&client, &server.base, "alias-bob").await;
    let outsider = register(&client, &server.base, "alias-outsider").await;

    let group: serde_json::Value = client
        .post(format!("{}/api/rooms", server.base))
        .bearer_auth(&alice.token)
        .json(&serde_json::json!({
            "name": "Original group",
            "password": "",
            "join_policy": "open"
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let group_id = group["id"].as_str().unwrap();
    assert_eq!(
        client
            .post(format!(
                "{}/api/rooms/{group_id}/join-requests",
                server.base
            ))
            .bearer_auth(&bob.token)
            .json(&serde_json::json!({}))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );

    let response = set_alias(
        &client,
        &server.base,
        &alice.token,
        group_id,
        "  Design crew  ",
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let updated: serde_json::Value = response.json().await.unwrap();
    assert_eq!(updated["alias"], "Design crew");
    assert_eq!(updated["title"], "Original group");

    assert_eq!(
        set_alias(&client, &server.base, &bob.token, group_id, "Weekly sync")
            .await
            .status(),
        StatusCode::OK
    );
    let alice_groups = conversations(&client, &server.base, &alice.token).await;
    let bob_groups = conversations(&client, &server.base, &bob.token).await;
    assert_eq!(
        conversation(&alice_groups, group_id)["alias"],
        "Design crew"
    );
    assert_eq!(conversation(&bob_groups, group_id)["alias"], "Weekly sync");
    assert_eq!(
        set_alias(
            &client,
            &server.base,
            &outsider.token,
            group_id,
            "Should fail"
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );

    assert!(client
        .post(format!("{}/api/friend-requests", server.base))
        .bearer_auth(&alice.token)
        .json(&serde_json::json!({ "user_id": bob.id }))
        .send()
        .await
        .unwrap()
        .status()
        .is_success());
    assert_eq!(
        client
            .patch(format!("{}/api/friend-requests/{}", server.base, alice.id))
            .bearer_auth(&bob.token)
            .json(&serde_json::json!({ "action": "accept" }))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    let direct: serde_json::Value = client
        .post(format!("{}/api/direct-chats", server.base))
        .bearer_auth(&alice.token)
        .json(&serde_json::json!({ "user_id": bob.id }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let direct_id = direct["room_id"].as_str().unwrap();
    assert_eq!(
        set_alias(&client, &server.base, &alice.token, direct_id, "Old friend")
            .await
            .status(),
        StatusCode::OK
    );
    let alice_direct = conversations(&client, &server.base, &alice.token).await;
    let bob_direct = conversations(&client, &server.base, &bob.token).await;
    assert_eq!(
        conversation(&alice_direct, direct_id)["alias"],
        "Old friend"
    );
    assert_eq!(conversation(&bob_direct, direct_id)["alias"], "");

    let cleared = set_alias(&client, &server.base, &alice.token, direct_id, "   ").await;
    assert_eq!(cleared.status(), StatusCode::OK);
    assert_eq!(
        cleared.json::<serde_json::Value>().await.unwrap()["alias"],
        ""
    );
    assert_eq!(
        set_alias(
            &client,
            &server.base,
            &alice.token,
            direct_id,
            &"x".repeat(65)
        )
        .await
        .status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        set_alias(&client, &server.base, &alice.token, direct_id, "bad\nalias")
            .await
            .status(),
        StatusCode::BAD_REQUEST
    );
}
