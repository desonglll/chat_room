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

async fn create_room(client: &Client, base: &str, token: &str, name: &str) -> String {
    client
        .post(format!("{base}/api/rooms"))
        .bearer_auth(token)
        .json(&serde_json::json!({ "name": name, "join_policy": "open" }))
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

async fn preferences(client: &Client, base: &str, token: &str, room_id: &str) -> reqwest::Response {
    client
        .get(format!("{base}/api/conversations/{room_id}/preferences"))
        .bearer_auth(token)
        .send()
        .await
        .unwrap()
}

async fn update_preferences(
    client: &Client,
    base: &str,
    token: &str,
    room_id: &str,
    body: serde_json::Value,
) -> reqwest::Response {
    client
        .patch(format!("{base}/api/conversations/{room_id}/preferences"))
        .bearer_auth(token)
        .json(&body)
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

#[tokio::test]
async fn preferences_are_private_partial_and_embedded_in_summaries() {
    let server = start_server().await;
    let client = Client::new();
    let alice = register(&client, &server.base, "preferences-alice").await;
    let bob = register(&client, &server.base, "preferences-bob").await;
    let room_id = create_room(&client, &server.base, &alice.token, "preferences-shared").await;
    assert_eq!(
        client
            .post(format!("{}/api/rooms/{room_id}/join-requests", server.base))
            .bearer_auth(&bob.token)
            .json(&serde_json::json!({}))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );

    let defaults: serde_json::Value = preferences(&client, &server.base, &alice.token, &room_id)
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(defaults["room_id"], room_id);
    assert_eq!(defaults["is_pinned"], false);
    assert_eq!(defaults["is_archived"], false);
    assert_eq!(defaults["notification_level"], "all");
    assert!(defaults["muted_until"].is_null());

    let updated: serde_json::Value = update_preferences(
        &client,
        &server.base,
        &alice.token,
        &room_id,
        serde_json::json!({
            "is_pinned": true,
            "notification_level": "mentions",
            "muted_until": "2030-01-02T03:04:05Z"
        }),
    )
    .await
    .json()
    .await
    .unwrap();
    assert_eq!(updated["is_pinned"], true);
    assert_eq!(updated["is_archived"], false);
    assert_eq!(updated["notification_level"], "mentions");
    assert_eq!(updated["muted_until"], "2030-01-02T03:04:05Z");

    let alice_items = conversations(&client, &server.base, &alice.token).await;
    let summary = alice_items
        .iter()
        .find(|item| item["room_id"] == room_id)
        .unwrap();
    assert_eq!(summary["preferences"]["is_pinned"], true);
    assert_eq!(summary["preferences"]["notification_level"], "mentions");
    let bob_defaults: serde_json::Value = preferences(&client, &server.base, &bob.token, &room_id)
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(bob_defaults["is_pinned"], false);
    assert_eq!(bob_defaults["notification_level"], "all");

    let cleared: serde_json::Value = update_preferences(
        &client,
        &server.base,
        &alice.token,
        &room_id,
        serde_json::json!({ "muted_until": null }),
    )
    .await
    .json()
    .await
    .unwrap();
    assert!(cleared["muted_until"].is_null());
    assert_eq!(cleared["is_pinned"], true);
}

#[tokio::test]
async fn pinned_and_archived_sorting_applies_to_groups_and_direct_conversations() {
    let server = start_server().await;
    let client = Client::new();
    let alice = register(&client, &server.base, "preferences-sort-alice").await;
    let bob = register(&client, &server.base, "preferences-sort-bob").await;
    let normal_id = create_room(&client, &server.base, &alice.token, "normal-room").await;
    let archived_id = create_room(&client, &server.base, &alice.token, "archived-room").await;

    client
        .post(format!("{}/api/friend-requests", server.base))
        .bearer_auth(&alice.token)
        .json(&serde_json::json!({ "user_id": bob.id }))
        .send()
        .await
        .unwrap();
    client
        .patch(format!("{}/api/friend-requests/{}", server.base, alice.id))
        .bearer_auth(&bob.token)
        .json(&serde_json::json!({ "action": "accept" }))
        .send()
        .await
        .unwrap();
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
        update_preferences(
            &client,
            &server.base,
            &alice.token,
            direct_id,
            serde_json::json!({ "is_pinned": true, "notification_level": "none" }),
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert_eq!(
        update_preferences(
            &client,
            &server.base,
            &alice.token,
            &archived_id,
            serde_json::json!({ "is_archived": true, "is_pinned": true }),
        )
        .await
        .status(),
        StatusCode::OK
    );

    let items = conversations(&client, &server.base, &alice.token).await;
    let position = |room_id: &str| {
        items
            .iter()
            .position(|item| item["room_id"] == room_id)
            .unwrap()
    };
    assert!(position(direct_id) < position(&normal_id));
    assert!(position(&normal_id) < position(&archived_id));
    assert_eq!(items[position(direct_id)]["kind"], "direct");
    assert_eq!(items[position(direct_id)]["unread_count"], 0);
    assert_eq!(
        items[position(&archived_id)]["preferences"]["is_pinned"],
        true
    );

    assert_eq!(
        client
            .delete(format!("{}/api/rooms/{normal_id}/members/me", server.base))
            .bearer_auth(&alice.token)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        preferences(&client, &server.base, &alice.token, &normal_id)
            .await
            .status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        update_preferences(
            &client,
            &server.base,
            &alice.token,
            &normal_id,
            serde_json::json!({ "is_pinned": true }),
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );
}
