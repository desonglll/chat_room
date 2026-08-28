use std::{path::Path, sync::Arc, time::Duration};

use chat_room::{
    backup, build_app,
    config::{AdminConfig, AppConfig, BackupConfig},
    state::AppState,
};
use reqwest::{multipart, Client, StatusCode};
use sqlx::sqlite::SqlitePoolOptions;
use tokio::net::TcpListener;
use uuid::Uuid;

mod support;
use support::system_admin_token;

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
    let response = client
        .post(format!("{base}/api/rooms"))
        .bearer_auth(token)
        .json(&serde_json::json!({ "name": name, "password": "" }))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    response.json().await.unwrap()
}

async fn run_backup(client: &Client, base: &str, token: &str) -> serde_json::Value {
    let response = client
        .post(format!("{base}/api/admin/backups/run"))
        .bearer_auth(token)
        .json(&serde_json::json!({ "include_files": false }))
        .send()
        .await
        .unwrap();
    let status = response.status();
    let body = response.text().await.unwrap();
    assert_eq!(status, StatusCode::OK, "backup response: {body}");
    serde_json::from_str(&body).unwrap()
}

fn restore_form(archive: Vec<u8>, confirmation: Option<&str>) -> multipart::Form {
    let form = multipart::Form::new().part(
        "file",
        multipart::Part::bytes(archive)
            .file_name("backup.tar.gz")
            .mime_str("application/gzip")
            .unwrap(),
    );
    match confirmation {
        Some(value) => form.text("confirmation", value.to_owned()),
        None => form,
    }
}

#[tokio::test]
async fn sqlite_backup_is_consistent_retained_and_requires_confirmed_restore() {
    let root = std::env::temp_dir().join(format!("chat-backup-automation-{}", Uuid::new_v4()));
    let database = root.join("source.db");
    let attachments = root.join("attachments");
    let destination = root.join("backups");
    let mut config = AppConfig::default();
    config.database.sqlite_path = database.clone();
    config.attachments.directory = attachments;
    config.admin = AdminConfig {
        usernames: vec!["backup-admin".into()],
        ..AdminConfig::default()
    };
    config.backup = BackupConfig {
        directory: destination,
        retention_count: 2,
        ..BackupConfig::default()
    };

    let state = Arc::new(
        AppState::open_with_config(&database, &config)
            .await
            .unwrap(),
    );
    let server = start(state.clone()).await;
    let client = Client::new();
    let admin = system_admin_token(&state, &server.base, "backup-admin").await;
    let room = create_room(&client, &server.base, &admin, "snapshot-room").await;
    let upload = client
        .post(format!(
            "{}/api/rooms/{}/attachments",
            server.base,
            room["id"].as_str().unwrap()
        ))
        .bearer_auth(&admin)
        .multipart(
            multipart::Form::new().part(
                "file",
                multipart::Part::bytes(b"restore drill".to_vec())
                    .file_name("drill.txt")
                    .mime_str("text/plain")
                    .unwrap(),
            ),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(upload.status(), StatusCode::CREATED);
    let mut expected_counts = Vec::new();
    for table in ["users", "rooms", "messages", "attachments"] {
        let count: i64 = sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {table}"))
            .fetch_one(state.pool())
            .await
            .unwrap();
        expected_counts.push((table, count));
    }

    let first = run_backup(&client, &server.base, &admin).await;
    assert_eq!(first["status"], "succeeded");
    assert_eq!(first["database_kind"], "sqlite");
    assert_eq!(first["checksum_status"], "verified");
    assert_eq!(first["artifact_sha256"].as_str().unwrap().len(), 64);
    let first_archive = Path::new(first["artifact_path"].as_str().unwrap()).to_path_buf();
    let archive_bytes = tokio::fs::read(&first_archive).await.unwrap();

    let extracted = root.join("restore-drill");
    backup::unpack_archive(&first_archive, &extracted).unwrap();
    let manifest = backup::read_and_verify(&extracted).unwrap();
    assert_eq!(manifest.database_kind, "sqlite");
    assert!(!manifest.includes_files);
    let restored_pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&format!(
            "sqlite://{}",
            extracted.join("database.dump").display()
        ))
        .await
        .unwrap();
    for (table, expected) in expected_counts {
        let restored: i64 = sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {table}"))
            .fetch_one(&restored_pool)
            .await
            .unwrap();
        assert_eq!(restored, expected, "restored {table} count");
    }
    restored_pool.close().await;

    create_room(&client, &server.base, &admin, "after-snapshot").await;
    let validated = client
        .post(format!("{}/api/admin/backups/restore", server.base))
        .bearer_auth(&admin)
        .multipart(restore_form(archive_bytes.clone(), None))
        .send()
        .await
        .unwrap();
    assert_eq!(validated.status(), StatusCode::OK);
    let validation: serde_json::Value = validated.json().await.unwrap();
    assert_eq!(validation["valid"], true);
    assert_eq!(validation["database_kind"], "sqlite");

    let unconfirmed = client
        .post(format!("{}/api/admin/backups/restore/execute", server.base))
        .bearer_auth(&admin)
        .multipart(restore_form(archive_bytes.clone(), None))
        .send()
        .await
        .unwrap();
    assert_eq!(unconfirmed.status(), StatusCode::BAD_REQUEST);

    let restored = client
        .post(format!("{}/api/admin/backups/restore/execute", server.base))
        .bearer_auth(&admin)
        .multipart(restore_form(archive_bytes, Some("RESTORE")))
        .send()
        .await
        .unwrap();
    let status = restored.status();
    let body = restored.text().await.unwrap();
    assert_eq!(status, StatusCode::OK, "restore response: {body}");
    assert_eq!(state.list_rooms(None).await.len(), 1);
    assert!(state.chat_rooms_locked().await.unwrap());

    let second = run_backup(&client, &server.base, &admin).await;
    let third = run_backup(&client, &server.base, &admin).await;
    assert!(Path::new(second["artifact_path"].as_str().unwrap()).exists());
    assert!(Path::new(third["artifact_path"].as_str().unwrap()).exists());
    assert!(!first_archive.exists());

    let status = client
        .get(format!("{}/api/admin/backups", server.base))
        .bearer_auth(&admin)
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();
    assert_eq!(status["target_backend"], "local");
    assert_eq!(status["retention_count"], 2);
    assert_eq!(status["runs"].as_array().unwrap().len(), 2);

    server.task.abort();
    state.pool().close().await;
    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test]
async fn scheduled_backup_failure_is_visible_to_administrators() {
    let root = std::env::temp_dir().join(format!("chat-backup-failure-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let invalid_destination = root.join("not-a-directory");
    std::fs::write(&invalid_destination, b"occupied").unwrap();
    let database = root.join("source.db");
    let mut config = AppConfig::default();
    config.admin.usernames = vec!["backup-admin".into()];
    config.attachments.directory = root.join("attachments");
    config.backup = BackupConfig {
        enabled: true,
        directory: invalid_destination,
        ..BackupConfig::default()
    };
    let state = Arc::new(
        AppState::open_with_config(&database, &config)
            .await
            .unwrap(),
    );
    let server = start(state.clone()).await;
    let client = Client::new();
    let admin = system_admin_token(&state, &server.base, "backup-admin").await;

    let mut failure = None;
    for _ in 0..40 {
        let status = client
            .get(format!("{}/api/admin/backups", server.base))
            .bearer_auth(&admin)
            .send()
            .await
            .unwrap()
            .json::<serde_json::Value>()
            .await
            .unwrap();
        failure = status["runs"]
            .as_array()
            .unwrap()
            .iter()
            .find(|run| run["status"] == "failed")
            .cloned();
        if failure.is_some() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let failure = failure.expect("scheduled failure should be persisted");
    assert_eq!(failure["trigger"], "scheduled");
    assert_eq!(failure["checksum_status"], "unavailable");
    assert!(failure["error"].as_str().unwrap().contains("服务器日志"));

    server.task.abort();
    state.pool().close().await;
    let _ = std::fs::remove_dir_all(root);
}
