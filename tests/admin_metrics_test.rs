use std::sync::Arc;

use chat_room::{
    ai::AiConfig,
    build_app,
    config::{AdminConfig, AppConfig, RedisConfig, VectorStoreConfig},
    state::AppState,
};
use chrono::{Duration, Utc};
use reqwest::multipart;
use tokio::net::TcpListener;
use uuid::Uuid;

mod support;
use support::session_token;

struct Server {
    base: String,
    state: Arc<AppState>,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn start() -> Server {
    let _ = tracing_subscriber::fmt().with_test_writer().try_init();
    let config = AppConfig {
        admin: AdminConfig {
            usernames: vec!["Ops-Admin".into()],
            orphan_retention_hours: 1,
            deleted_room_retention_days: 1,
        },
        ..AppConfig::default()
    };
    let state = Arc::new(AppState::new_with_config(&config).await.unwrap());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn({
        let state = state.clone();
        async move { axum::serve(listener, build_app(state)).await.unwrap() }
    });
    Server { base, state, task }
}

async fn create_room(base: &str, token: &str, name: &str) -> serde_json::Value {
    reqwest::Client::new()
        .post(format!("{base}/api/rooms"))
        .bearer_auth(token)
        .json(&serde_json::json!({ "name": name, "password": "" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap()
}

#[tokio::test]
async fn overview_requires_allowlisted_authenticated_account() {
    let server = start().await;
    let client = reqwest::Client::new();
    let regular = session_token(&server.base, "regular-user").await;
    let admin = session_token(&server.base, "ops-admin").await;
    create_room(&server.base, &admin, "admin-visible-room").await;

    assert_eq!(
        client
            .get(format!("{}/api/admin/overview", server.base))
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    assert_eq!(
        client
            .get(format!("{}/api/admin/overview", server.base))
            .bearer_auth(&regular)
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    let response = client
        .get(format!("{}/api/admin/overview", server.base))
        .bearer_auth(&admin)
        .send()
        .await
        .unwrap();
    let status = response.status();
    let body = response.text().await.unwrap();
    assert_eq!(status, 200, "overview response: {body}");
    let overview: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(overview["database_backend"], "sqlite");
    assert_eq!(overview["attachment_backend"], "local");
    assert_eq!(overview["chat_rooms_locked"], false);
    assert_eq!(overview["totals"]["users"], 2);
    assert_eq!(overview["totals"]["active_rooms"], 1);
    assert!(overview["runtime"]["requests"].as_u64().unwrap() >= 2);
    assert_eq!(overview["top_rooms"][0]["name"], "admin-visible-room");
    assert_eq!(overview["services"]["items"][0]["id"], "database");
    assert_eq!(overview["services"]["items"][0]["state"], "healthy");
    assert_eq!(overview["services"]["items"][1]["id"], "redis");
    assert_eq!(overview["services"]["items"][1]["state"], "disabled");
    assert_eq!(overview["services"]["items"][2]["id"], "vector_store");
    assert_eq!(overview["services"]["items"][2]["state"], "disabled");
    assert_eq!(overview["services"]["items"][4]["id"], "ai_provider");
    assert_eq!(overview["services"]["items"][4]["state"], "disabled");
    assert_eq!(overview["services"]["vector_index"]["pending_jobs"], 0);
}

#[tokio::test]
async fn unavailable_vector_store_is_reported_without_stopping_the_server() {
    let config = AppConfig {
        admin: AdminConfig {
            usernames: vec!["vector-admin".into()],
            ..AdminConfig::default()
        },
        vector_store: VectorStoreConfig {
            enabled: true,
            url: "http://127.0.0.1:9".into(),
            collection: "messages".into(),
            dimensions: 2,
            embedding_base_url: "http://127.0.0.1:9/v1".into(),
            embedding_model: "embed-test".into(),
            ..VectorStoreConfig::default()
        },
        redis: RedisConfig {
            enabled: true,
            url: "redis://127.0.0.1:9/".into(),
            connect_timeout_ms: 100,
            command_timeout_ms: 100,
            ..RedisConfig::default()
        },
        ai: AiConfig {
            enabled: true,
            provider: "openai-compatible".into(),
            api_key_env: "CHAT_ROOM_TEST_MISSING_AI_KEY".into(),
            model: "test-model".into(),
            ..AiConfig::default()
        },
        ..AppConfig::default()
    };
    let state = Arc::new(
        AppState::new_with_config(&config)
            .await
            .expect("vector dependency failure should degrade instead of aborting startup"),
    );
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn({
        let state = state.clone();
        async move { axum::serve(listener, build_app(state)).await.unwrap() }
    });
    let server = Server { base, state, task };
    let admin = session_token(&server.base, "vector-admin").await;
    let overview: serde_json::Value = reqwest::Client::new()
        .get(format!("{}/api/admin/overview", server.base))
        .bearer_auth(admin)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let services = overview["services"]["items"].as_array().unwrap();
    for id in ["vector_store", "redis", "ai_provider"] {
        let service = services.iter().find(|item| item["id"] == id).unwrap();
        assert_eq!(service["state"], "degraded", "service: {id}");
    }
}

#[tokio::test]
async fn purge_removes_only_data_older_than_configured_retention() {
    let server = start().await;
    let client = reqwest::Client::new();
    let admin = session_token(&server.base, "OPS-ADMIN").await;
    let room = create_room(&server.base, &admin, "retention-room").await;
    let room_id = Uuid::parse_str(room["id"].as_str().unwrap()).unwrap();
    let part = multipart::Part::bytes(b"expired-orphan".to_vec())
        .file_name("expired.bin")
        .mime_str("application/octet-stream")
        .unwrap();
    let uploaded: serde_json::Value = client
        .post(format!("{}/api/rooms/{room_id}/attachments", server.base))
        .bearer_auth(&admin)
        .multipart(multipart::Form::new().part("file", part))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let attachment_id = Uuid::parse_str(uploaded["attachment"]["id"].as_str().unwrap()).unwrap();
    let storage_key: String =
        sqlx::query_scalar("SELECT storage_key FROM attachments WHERE id = ?")
            .bind(attachment_id)
            .fetch_one(server.state.pool())
            .await
            .unwrap();
    sqlx::query("UPDATE messages SET recalled_at = ? WHERE attachment_id = ?")
        .bind(Utc::now() - Duration::hours(2))
        .bind(attachment_id)
        .execute(server.state.pool())
        .await
        .unwrap();
    sqlx::query("UPDATE attachments SET orphaned_at = ? WHERE id = ?")
        .bind(Utc::now() - Duration::hours(2))
        .bind(attachment_id)
        .execute(server.state.pool())
        .await
        .unwrap();

    let result: serde_json::Value = client
        .post(format!("{}/api/admin/maintenance/purge", server.base))
        .bearer_auth(&admin)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(result["attachment_objects_deleted"], 1);
    assert_eq!(result["rooms_deleted"], 0);
    let row_exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM attachments WHERE id = ?)")
            .bind(attachment_id)
            .fetch_one(server.state.pool())
            .await
            .unwrap();
    assert!(!row_exists);
    assert!(!server
        .state
        .attachment_store()
        .exists(&storage_key)
        .await
        .unwrap());

    client
        .delete(format!("{}/api/rooms/{room_id}", server.base))
        .bearer_auth(&admin)
        .send()
        .await
        .unwrap();
    sqlx::query("UPDATE rooms SET deleted_at = ? WHERE id = ?")
        .bind(Utc::now() - Duration::days(2))
        .bind(room_id)
        .execute(server.state.pool())
        .await
        .unwrap();
    let result: serde_json::Value = client
        .post(format!("{}/api/admin/maintenance/purge", server.base))
        .bearer_auth(&admin)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(result["rooms_deleted"], 1);
}

#[tokio::test]
async fn purge_preserves_a_deleted_room_while_its_video_is_favorited() {
    let server = start().await;
    let client = reqwest::Client::new();
    let admin = session_token(&server.base, "ops-admin").await;
    let room = create_room(&server.base, &admin, "favorited-retention-room").await;
    let room_id = Uuid::parse_str(room["id"].as_str().unwrap()).unwrap();
    let part = multipart::Part::bytes(b"favorite-video".to_vec())
        .file_name("preserved.mp4")
        .mime_str("video/mp4")
        .unwrap();
    let uploaded: serde_json::Value = client
        .post(format!("{}/api/rooms/{room_id}/attachments", server.base))
        .bearer_auth(&admin)
        .multipart(multipart::Form::new().part("file", part))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let attachment_id = Uuid::parse_str(uploaded["attachment"]["id"].as_str().unwrap()).unwrap();
    let message_id = uploaded["id"].as_str().unwrap();
    let favorites: Vec<serde_json::Value> = client
        .post(format!("{}/api/favorites/messages", server.base))
        .bearer_auth(&admin)
        .json(&serde_json::json!({ "message_ids": [message_id] }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    client
        .delete(format!("{}/api/rooms/{room_id}", server.base))
        .bearer_auth(&admin)
        .send()
        .await
        .unwrap();
    sqlx::query("UPDATE rooms SET deleted_at = ? WHERE id = ?")
        .bind(Utc::now() - Duration::days(2))
        .bind(room_id)
        .execute(server.state.pool())
        .await
        .unwrap();
    let preserved: serde_json::Value = client
        .post(format!("{}/api/admin/maintenance/purge", server.base))
        .bearer_auth(&admin)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(preserved["rooms_deleted"], 0);
    let attachment_exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM attachments WHERE id = ?)")
            .bind(attachment_id)
            .fetch_one(server.state.pool())
            .await
            .unwrap();
    assert!(attachment_exists);

    client
        .delete(format!(
            "{}/api/favorites/{}",
            server.base,
            favorites[0]["id"].as_str().unwrap()
        ))
        .bearer_auth(&admin)
        .send()
        .await
        .unwrap();
    let removed: serde_json::Value = client
        .post(format!("{}/api/admin/maintenance/purge", server.base))
        .bearer_auth(&admin)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(removed["rooms_deleted"], 1);
}
