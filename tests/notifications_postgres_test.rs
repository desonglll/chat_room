use std::sync::Arc;

use chat_room::{build_app, config::AppConfig, state::AppState};
use chrono::Utc;
use reqwest::Client;
use sqlx::postgres::PgPoolOptions;
use tokio::net::TcpListener;
use uuid::Uuid;

struct Account {
    id: Uuid,
    token: String,
}

async fn postgres_admin() -> Option<(String, sqlx::PgPool)> {
    let configured = std::env::var("TEST_POSTGRES_ADMIN_URL").ok();
    let url = configured
        .clone()
        .unwrap_or_else(|| "postgresql://postgres:postgres@localhost:52735/postgres".into());
    match PgPoolOptions::new().max_connections(1).connect(&url).await {
        Ok(pool) => Some((url, pool)),
        Err(error) if configured.is_some() => {
            panic!("required PostgreSQL at {url} is unavailable: {error}")
        }
        Err(error) => {
            eprintln!("skipping PostgreSQL notification test: {error}");
            None
        }
    }
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

async fn notifications(client: &Client, base: &str, token: &str) -> serde_json::Value {
    client
        .get(format!("{base}/api/notifications"))
        .bearer_auth(token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap()
}

#[tokio::test]
async fn postgres_notifications_match_trigger_and_authorization_contracts() {
    let Some((admin_url, admin_pool)) = postgres_admin().await else {
        return;
    };
    let database_name = format!("chat_room_notifications_{}", Uuid::new_v4().simple());
    sqlx::query(&format!(r#"CREATE DATABASE "{database_name}""#))
        .execute(&admin_pool)
        .await
        .unwrap();
    let database_url = format!(
        "{}/{}",
        admin_url.rsplit_once('/').unwrap().0,
        database_name
    );
    let state = Arc::new(
        AppState::open_postgres(&database_url, &AppConfig::default())
            .await
            .unwrap(),
    );
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn({
        let state = state.clone();
        async move { axum::serve(listener, build_app(state)).await.unwrap() }
    });
    let client = Client::new();
    let alice = register(&client, &base, "pg-notify-alice").await;
    let bob = register(&client, &base, "pg-notify-bob").await;

    client
        .post(format!("{base}/api/friend-requests"))
        .bearer_auth(&alice.token)
        .json(&serde_json::json!({ "user_id": bob.id }))
        .send()
        .await
        .unwrap();
    let bob_page = notifications(&client, &base, &bob.token).await;
    assert_eq!(bob_page["items"].as_array().unwrap().len(), 1);
    assert_eq!(bob_page["items"][0]["kind"], "friend_request");
    assert_eq!(bob_page["items"][0]["actor"]["id"], alice.id.to_string());

    let room: serde_json::Value = client
        .post(format!("{base}/api/rooms"))
        .bearer_auth(&alice.token)
        .json(&serde_json::json!({ "name": "pg-notification-room", "join_policy": "open" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let room_id: Uuid = room["id"].as_str().unwrap().parse().unwrap();
    client
        .post(format!("{base}/api/rooms/{room_id}/join-requests"))
        .bearer_auth(&bob.token)
        .json(&serde_json::json!({}))
        .send()
        .await
        .unwrap();
    let message_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO messages (id, room_id, sender_id, sender, content, created_at) \
         VALUES ($1, $2, $3, 'pg-notify-alice', 'postgres mention source', $4)",
    )
    .bind(message_id)
    .bind(room_id)
    .bind(alice.id)
    .bind(Utc::now())
    .execute(state.postgres_pool().unwrap())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO message_mentions (message_id, mentioned_user_id, created_at) \
         VALUES ($1, $2, $3)",
    )
    .bind(message_id)
    .bind(bob.id)
    .bind(Utc::now())
    .execute(state.postgres_pool().unwrap())
    .await
    .unwrap();
    let bob_page = notifications(&client, &base, &bob.token).await;
    assert_eq!(bob_page["items"][0]["kind"], "mention");
    assert_eq!(bob_page["items"][0]["summary"], "postgres mention source");
    assert_eq!(bob_page["items"][0]["source_available"], true);

    sqlx::query("UPDATE messages SET recalled_at = $1 WHERE id = $2")
        .bind(Utc::now())
        .bind(message_id)
        .execute(state.postgres_pool().unwrap())
        .await
        .unwrap();
    let bob_page = notifications(&client, &base, &bob.token).await;
    assert_eq!(bob_page["items"][0]["source_available"], false);
    assert!(bob_page["items"][0]["message_id"].is_null());

    task.abort();
    let _ = task.await;
    state.postgres_pool().unwrap().close().await;
    drop(state);
    sqlx::query("SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE datname = $1")
        .bind(&database_name)
        .execute(&admin_pool)
        .await
        .ok();
    sqlx::query(&format!(r#"DROP DATABASE "{database_name}""#))
        .execute(&admin_pool)
        .await
        .unwrap();
}
