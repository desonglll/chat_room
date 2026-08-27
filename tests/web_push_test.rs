use std::sync::Arc;

use chat_room::{
    build_app,
    config::AppConfig,
    notifications::{NotificationEvent, NotificationKind},
    state::AppState,
};
use chrono::Utc;
use reqwest::{Client, StatusCode};
use tokio::net::TcpListener;

struct TestServer {
    base: String,
    state: Arc<AppState>,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

struct Account {
    id: uuid::Uuid,
    token: String,
}

async fn start_server() -> TestServer {
    let mut config = AppConfig::default();
    config.web_push.enabled = true;
    config.web_push.public_key = "test-public-key".into();
    config.web_push.private_key = "IQ9Ur0ykXoHS9gzfYX0aBjy9lvdrjx_PFUXmie9YRcY".into();
    config.web_push.subject = "mailto:test@example.com".into();
    config.web_push.poll_interval_ms = 60_000;
    config.web_push.allowed_endpoint_hosts = vec!["push.example".into()];
    let state = Arc::new(AppState::new_with_config(&config).await.unwrap());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let app = build_app(state.clone());
    let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    TestServer { base, state, task }
}

async fn register(client: &Client, base: &str, username: &str) -> Account {
    let value: serde_json::Value = client
        .post(format!("{base}/api/users/register"))
        .json(&serde_json::json!({ "username": username, "password": "test-password" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    Account {
        id: value["user"]["id"].as_str().unwrap().parse().unwrap(),
        token: value["token"].as_str().unwrap().into(),
    }
}

async fn subscribe(
    client: &Client,
    server: &TestServer,
    account: &Account,
    endpoint: &str,
) -> reqwest::Response {
    client
        .post(format!("{}/api/push/subscriptions", server.base))
        .bearer_auth(&account.token)
        .json(&serde_json::json!({
            "endpoint": endpoint,
            "keys": { "p256dh": "device-public-key", "auth": "device-auth" },
            "show_details": false
        }))
        .send()
        .await
        .unwrap()
}

#[tokio::test]
async fn subscriptions_are_device_scoped_and_create_durable_delivery_jobs() {
    let server = start_server().await;
    let client = Client::new();
    let config: serde_json::Value = client
        .get(format!("{}/api/push/config", server.base))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(config["enabled"], true);
    assert_eq!(config["public_key"], "test-public-key");
    assert!(config.get("private_key").is_none());

    let alice = register(&client, &server.base, "push-alice").await;
    let bob = register(&client, &server.base, "push-bob").await;
    let first_endpoint = "https://push.example/device-one";
    let first = subscribe(&client, &server, &alice, first_endpoint).await;
    assert_eq!(first.status(), StatusCode::OK);
    let body: serde_json::Value = first.json().await.unwrap();
    assert!(body.get("endpoint").is_none());

    // Presenting the exact endpoint and key material proves the same browser
    // subscription and safely transfers it after an account switch.
    assert_eq!(
        subscribe(&client, &server, &bob, first_endpoint)
            .await
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        subscribe(&client, &server, &bob, "https://push.example/device-two")
            .await
            .status(),
        StatusCode::OK
    );

    server
        .state
        .record_notification(&NotificationEvent {
            recipient_id: bob.id,
            kind: NotificationKind::FriendRequest,
            actor_id: Some(alice.id),
            room_id: None,
            message_id: None,
            run_id: None,
            summary: String::new(),
            dedupe_key: "push-http-notification".into(),
            created_at: Utc::now(),
        })
        .await
        .unwrap();
    let jobs: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM push_delivery_jobs")
        .fetch_one(server.state.pool())
        .await
        .unwrap();
    assert_eq!(jobs, 2);

    let delete = client
        .delete(format!("{}/api/push/subscriptions", server.base))
        .bearer_auth(&bob.token)
        .json(&serde_json::json!({ "endpoint": first_endpoint }))
        .send()
        .await
        .unwrap();
    assert_eq!(delete.status(), StatusCode::NO_CONTENT);
    let remaining: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM push_subscriptions")
        .fetch_one(server.state.pool())
        .await
        .unwrap();
    assert_eq!(remaining, 1);
}
