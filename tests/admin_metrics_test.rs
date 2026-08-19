use std::sync::Arc;

use chat_room::{
    build_app,
    config::{AdminConfig, AppConfig},
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
    assert_eq!(overview["totals"]["users"], 2);
    assert_eq!(overview["totals"]["active_rooms"], 1);
    assert!(overview["runtime"]["requests"].as_u64().unwrap() >= 2);
    assert_eq!(overview["top_rooms"][0]["name"], "admin-visible-room");
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
