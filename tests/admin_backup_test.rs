use std::{path::Path, sync::Arc};

use chat_room::{
    build_app,
    config::{AdminConfig, AppConfig},
    state::AppState,
};
use reqwest::{multipart, Client, StatusCode};
use sqlx::postgres::PgPoolOptions;
use tokio::net::TcpListener;
use uuid::Uuid;

mod support;
use support::session_token;

struct Server {
    base: String,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn start(state: Arc<AppState>) -> Server {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        axum::serve(listener, build_app(state)).await.unwrap();
    });
    Server { base, task }
}

async fn create_room(client: &Client, base: &str, token: &str, name: &str) -> serde_json::Value {
    client
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
async fn backup_endpoints_require_admin_and_report_sqlite_as_unsupported() {
    let config = AppConfig {
        admin: AdminConfig {
            usernames: vec!["backup-admin".into()],
            ..AdminConfig::default()
        },
        ..AppConfig::default()
    };
    let server = start(Arc::new(AppState::new_with_config(&config).await.unwrap())).await;
    let client = Client::new();
    let regular = session_token(&server.base, "backup-regular").await;
    let admin = session_token(&server.base, "backup-admin").await;

    assert_eq!(
        client
            .post(format!("{}/api/admin/backups/export", server.base))
            .json(&serde_json::json!({ "include_files": false }))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        client
            .post(format!("{}/api/admin/backups/export", server.base))
            .bearer_auth(&regular)
            .json(&serde_json::json!({ "include_files": false }))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    let unsupported = client
        .post(format!("{}/api/admin/backups/export", server.base))
        .bearer_auth(&admin)
        .json(&serde_json::json!({ "include_files": false }))
        .send()
        .await
        .unwrap();
    assert_eq!(unsupported.status(), StatusCode::CONFLICT);
    assert!(
        unsupported.json::<serde_json::Value>().await.unwrap()["error"]
            .as_str()
            .unwrap()
            .contains("PostgreSQL")
    );
}

#[tokio::test]
async fn postgres_admin_can_export_and_restore_database_with_local_files() {
    let configured = std::env::var("TEST_POSTGRES_ADMIN_URL").ok();
    let admin_url = configured
        .clone()
        .unwrap_or_else(|| "postgresql://postgres:postgres@localhost:52735/postgres".into());
    let admin_pool = match PgPoolOptions::new()
        .max_connections(1)
        .connect(&admin_url)
        .await
    {
        Ok(pool) => pool,
        Err(error) if configured.is_some() => {
            panic!("required PostgreSQL at {admin_url} is unavailable: {error}")
        }
        Err(error) => {
            eprintln!("skipping backup restore test; PostgreSQL is unavailable: {error}");
            return;
        }
    };
    let database_name = format!("chat_room_backup_test_{}", Uuid::new_v4().simple());
    sqlx::query(&format!(r#"CREATE DATABASE "{database_name}""#))
        .execute(&admin_pool)
        .await
        .unwrap();
    let database_url = format!(
        "{}/{}",
        admin_url.rsplit_once('/').unwrap().0,
        database_name
    );
    let attachment_root =
        std::env::temp_dir().join(format!("chat-backup-files-{}", Uuid::new_v4()));
    let mut config = AppConfig::default();
    config.database.kind = "postgres".into();
    config.database.postgres_url = database_url.clone();
    config.attachments.directory = attachment_root.clone();
    config.admin.usernames = vec!["backup-admin".into()];

    let state = Arc::new(
        AppState::open_postgres(&database_url, &config)
            .await
            .unwrap(),
    );
    let server = start(state.clone()).await;
    let client = Client::new();
    let admin = session_token(&server.base, "backup-admin").await;
    let invalid_restore = restore_archive(
        &client,
        &server.base,
        &admin,
        b"not a backup archive".to_vec(),
    )
    .await;
    assert_eq!(invalid_restore.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let room = create_room(&client, &server.base, &admin, "before-backup").await;
    let room_id = room["id"].as_str().unwrap();
    let attachment_bytes = b"file preserved by complete backup";
    let upload = client
        .post(format!("{}/api/rooms/{room_id}/attachments", server.base))
        .bearer_auth(&admin)
        .multipart(
            multipart::Form::new().part(
                "file",
                multipart::Part::bytes(attachment_bytes.to_vec())
                    .file_name("proof.txt")
                    .mime_str("text/plain")
                    .unwrap(),
            ),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(upload.status(), StatusCode::CREATED);

    let data_export = client
        .post(format!("{}/api/admin/backups/export", server.base))
        .bearer_auth(&admin)
        .json(&serde_json::json!({ "include_files": false }))
        .send()
        .await
        .unwrap();
    assert_eq!(data_export.status(), StatusCode::OK);
    let data_archive = data_export.bytes().await.unwrap();
    create_room(&client, &server.base, &admin, "after-data-backup").await;
    let retained_file = attachment_root.join("retained-during-data-restore.txt");
    std::fs::write(&retained_file, b"current file stays").unwrap();

    let data_restored = restore_archive(&client, &server.base, &admin, data_archive.to_vec()).await;
    let status = data_restored.status();
    let body = data_restored.text().await.unwrap();
    assert_eq!(status, StatusCode::OK, "data restore response: {body}");
    let result: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(result["included_files"], false);
    assert_eq!(result["vector_messages_queued"], 0);
    assert!(retained_file.exists());
    assert_eq!(state.list_rooms(None).await.len(), 1);
    let unlocked = client
        .put(format!("{}/api/admin/chat-lock", server.base))
        .bearer_auth(&admin)
        .json(&serde_json::json!({ "locked": false }))
        .send()
        .await
        .unwrap();
    assert_eq!(unlocked.status(), StatusCode::OK);

    let full_export = client
        .post(format!("{}/api/admin/backups/export", server.base))
        .bearer_auth(&admin)
        .json(&serde_json::json!({ "include_files": true }))
        .send()
        .await
        .unwrap();
    assert_eq!(full_export.status(), StatusCode::OK);
    assert_eq!(full_export.headers()["content-type"], "application/gzip");
    let archive = full_export.bytes().await.unwrap();
    assert!(!archive.is_empty());

    create_room(&client, &server.base, &admin, "after-backup").await;
    remove_visible_entries(&attachment_root);
    assert!(find_file_with_bytes(&attachment_root, attachment_bytes).is_none());

    let restored = restore_archive(&client, &server.base, &admin, archive.to_vec()).await;
    let status = restored.status();
    let body = restored.text().await.unwrap();
    assert_eq!(status, StatusCode::OK, "restore response: {body}");
    let result: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(result["included_files"], true);
    assert_eq!(result["chat_rooms_locked"], true);
    assert_eq!(result["previous_files_preserved"], true);
    assert_eq!(result["vector_messages_queued"], 0);
    assert_eq!(
        state
            .list_rooms(None)
            .await
            .into_iter()
            .map(|room| room.name)
            .collect::<Vec<_>>(),
        vec!["before-backup"]
    );
    assert!(find_file_with_bytes(&attachment_root, attachment_bytes).is_some());

    server.task.abort();
    state.postgres_pool().unwrap().close().await;
    drop(state);
    sqlx::query("SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE datname = $1")
        .bind(&database_name)
        .execute(&admin_pool)
        .await
        .ok();
    sqlx::query(&format!(r#"DROP DATABASE IF EXISTS "{database_name}""#))
        .execute(&admin_pool)
        .await
        .unwrap();
    let _ = std::fs::remove_dir_all(attachment_root);
}

async fn restore_archive(
    client: &Client,
    base: &str,
    token: &str,
    archive: Vec<u8>,
) -> reqwest::Response {
    client
        .post(format!("{base}/api/admin/backups/restore"))
        .bearer_auth(token)
        .multipart(
            multipart::Form::new().part(
                "file",
                multipart::Part::bytes(archive)
                    .file_name("backup.tar.gz")
                    .mime_str("application/gzip")
                    .unwrap(),
            ),
        )
        .send()
        .await
        .unwrap()
}

fn remove_visible_entries(root: &Path) {
    for entry in std::fs::read_dir(root).unwrap() {
        let entry = entry.unwrap();
        if entry.file_name().to_string_lossy().starts_with('.') {
            continue;
        }
        if entry.file_type().unwrap().is_dir() {
            std::fs::remove_dir_all(entry.path()).unwrap();
        } else {
            std::fs::remove_file(entry.path()).unwrap();
        }
    }
}

fn find_file_with_bytes(root: &Path, expected: &[u8]) -> Option<std::path::PathBuf> {
    for entry in std::fs::read_dir(root).ok()? {
        let entry = entry.ok()?;
        if entry.file_type().ok()?.is_dir() {
            if let Some(path) = find_file_with_bytes(&entry.path(), expected) {
                return Some(path);
            }
        } else if std::fs::read(entry.path()).ok().as_deref() == Some(expected) {
            return Some(entry.path());
        }
    }
    None
}
