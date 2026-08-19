use std::sync::Arc;

use chat_room::{
    build_app,
    config::{AppConfig, AttachmentConfig},
    models::Room,
    state::AppState,
};
use chrono::Utc;
use futures_util::future::join_all;
use reqwest::multipart;
use sha2::{Digest, Sha256};
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
    let state = Arc::new(AppState::new().await.unwrap());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn({
        let state = state.clone();
        async move { axum::serve(listener, build_app(state)).await.unwrap() }
    });
    Server { base, state, task }
}

async fn room_for(base: &str, username: &str, room_name: &str) -> (String, String) {
    let token = session_token(base, username).await;
    let room: serde_json::Value = reqwest::Client::new()
        .post(format!("{base}/api/rooms"))
        .bearer_auth(&token)
        .json(&serde_json::json!({ "name": room_name, "password": "" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    (room["id"].as_str().unwrap().to_string(), token)
}

async fn create_room(base: &str, token: &str, room_name: &str) -> String {
    let room: serde_json::Value = reqwest::Client::new()
        .post(format!("{base}/api/rooms"))
        .bearer_auth(token)
        .json(&serde_json::json!({ "name": room_name, "password": "" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    room["id"].as_str().unwrap().to_string()
}

async fn upload(
    base: &str,
    room_id: &str,
    token: &str,
    name: &str,
    bytes: Vec<u8>,
) -> serde_json::Value {
    let part = multipart::Part::bytes(bytes)
        .file_name(name.to_string())
        .mime_str("application/octet-stream")
        .unwrap();
    let response = reqwest::Client::new()
        .post(format!("{base}/api/rooms/{room_id}/attachments"))
        .bearer_auth(token)
        .multipart(multipart::Form::new().part("file", part))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 201);
    response.json().await.unwrap()
}

async fn upload_chunked(
    base: &str,
    room_id: &str,
    token: &str,
    name: &str,
    bytes: &[u8],
) -> serde_json::Value {
    let client = reqwest::Client::new();
    let session: serde_json::Value = client
        .post(format!("{base}/api/rooms/{room_id}/attachments/uploads"))
        .bearer_auth(token)
        .json(&serde_json::json!({
            "file_name": name,
            "mime_type": "application/octet-stream",
            "size_bytes": bytes.len(),
            "fingerprint": format!("{name}:{}", bytes.len()),
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let upload_id = session["upload_id"].as_str().unwrap();
    for (offset, chunk) in bytes.chunks(5).enumerate() {
        let response = client
            .put(format!(
                "{base}/api/attachments/uploads/{upload_id}/chunks?offset={}",
                offset * 5
            ))
            .bearer_auth(token)
            .body(chunk.to_vec())
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
    }
    let response = client
        .post(format!(
            "{base}/api/attachments/uploads/{upload_id}/complete"
        ))
        .bearer_auth(token)
        .json(&serde_json::json!({ "content": "", "is_sensitive": false }))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 201);
    response.json().await.unwrap()
}

#[tokio::test]
async fn identical_uploads_share_one_physical_object() {
    let server = start().await;
    let (room_id, token) = room_for(&server.base, "hash-owner", "hash-room").await;
    let bytes = b"same-content-for-sequential-dedup".to_vec();
    let first = upload(&server.base, &room_id, &token, "first.bin", bytes.clone()).await;
    let second = upload(&server.base, &room_id, &token, "second.bin", bytes.clone()).await;

    let first_id = Uuid::parse_str(first["attachment"]["id"].as_str().unwrap()).unwrap();
    let second_id = Uuid::parse_str(second["attachment"]["id"].as_str().unwrap()).unwrap();
    let first_key: String = sqlx::query_scalar("SELECT storage_key FROM attachments WHERE id = ?")
        .bind(first_id)
        .fetch_one(server.state.pool())
        .await
        .unwrap();
    let second_key: String = sqlx::query_scalar("SELECT storage_key FROM attachments WHERE id = ?")
        .bind(second_id)
        .fetch_one(server.state.pool())
        .await
        .unwrap();
    assert_ne!(first_id, second_id);
    assert_eq!(first_key, second_key);
    assert!(server
        .state
        .attachment_store()
        .exists(&first_key)
        .await
        .unwrap());
}

#[tokio::test]
async fn concurrent_identical_uploads_are_race_safe() {
    let server = start().await;
    let (room_id, token) = room_for(&server.base, "race-owner", "race-room").await;
    let bytes = b"same-content-from-many-concurrent-requests".to_vec();
    let uploads = (0..16).map(|index| {
        let base = server.base.clone();
        let room_id = room_id.clone();
        let token = token.clone();
        let bytes = bytes.clone();
        async move {
            let name = format!("race-{index}.bin");
            upload(&base, &room_id, &token, &name, bytes).await
        }
    });
    let results = join_all(uploads).await;
    assert_eq!(results.len(), 16);

    let (rows, keys, hashes): (i64, i64, i64) = sqlx::query_as(
        "SELECT COUNT(*), COUNT(DISTINCT storage_key), COUNT(DISTINCT content_hash) \
         FROM attachments WHERE room_id = ?",
    )
    .bind(Uuid::parse_str(&room_id).unwrap())
    .fetch_one(server.state.pool())
    .await
    .unwrap();
    assert_eq!((rows, keys, hashes), (16, 1, 1));
}

#[tokio::test]
async fn chunked_upload_reuses_single_shot_content() {
    let server = start().await;
    let (room_id, token) = room_for(&server.base, "chunk-owner", "chunk-room").await;
    let bytes = b"content-shared-by-both-upload-protocols".to_vec();
    let first = upload(&server.base, &room_id, &token, "single.bin", bytes.clone()).await;
    let second = upload_chunked(&server.base, &room_id, &token, "chunked.bin", &bytes).await;
    let first_id = Uuid::parse_str(first["attachment"]["id"].as_str().unwrap()).unwrap();
    let second_id = Uuid::parse_str(second["attachment"]["id"].as_str().unwrap()).unwrap();
    let keys: Vec<String> = sqlx::query_scalar(
        "SELECT storage_key FROM attachments WHERE id IN (?, ?) ORDER BY storage_key",
    )
    .bind(first_id)
    .bind(second_id)
    .fetch_all(server.state.pool())
    .await
    .unwrap();
    assert_eq!(keys.len(), 2);
    assert_eq!(keys[0], keys[1]);
}

#[tokio::test]
async fn new_reference_clears_orphan_marker_for_shared_content() {
    let server = start().await;
    let (room_id, token) = room_for(&server.base, "revive-owner", "revive-room").await;
    let bytes = b"orphaned-content-referenced-again".to_vec();
    let first = upload(&server.base, &room_id, &token, "old.bin", bytes.clone()).await;
    let first_id = Uuid::parse_str(first["attachment"]["id"].as_str().unwrap()).unwrap();
    sqlx::query("UPDATE attachments SET orphaned_at = CURRENT_TIMESTAMP WHERE id = ?")
        .bind(first_id)
        .execute(server.state.pool())
        .await
        .unwrap();

    upload(&server.base, &room_id, &token, "new.bin", bytes).await;
    let marked: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM attachments WHERE room_id = ? AND orphaned_at IS NOT NULL",
    )
    .bind(Uuid::parse_str(&room_id).unwrap())
    .fetch_one(server.state.pool())
    .await
    .unwrap();
    assert_eq!(marked, 0);
}

#[tokio::test]
async fn forwarded_message_keeps_content_referenced_until_every_copy_is_recalled() {
    let server = start().await;
    let (source_room, token) = room_for(&server.base, "forward-owner", "forward-source-room").await;
    let target_room = create_room(&server.base, &token, "forward-target-room").await;
    let uploaded = upload(
        &server.base,
        &source_room,
        &token,
        "forward.bin",
        b"forwarded-content".to_vec(),
    )
    .await;
    let source_message = Uuid::parse_str(uploaded["id"].as_str().unwrap()).unwrap();
    let attachment_id = Uuid::parse_str(uploaded["attachment"]["id"].as_str().unwrap()).unwrap();
    let user = server
        .state
        .user_credentials("forward-owner")
        .await
        .unwrap()
        .unwrap()
        .0;
    let forwarded = server
        .state
        .forward_message(
            source_message,
            Uuid::parse_str(&source_room).unwrap(),
            Uuid::parse_str(&target_room).unwrap(),
            &user,
        )
        .await
        .unwrap()
        .unwrap();

    server
        .state
        .recall_message(
            Uuid::parse_str(&source_room).unwrap(),
            user.id,
            source_message,
        )
        .await
        .unwrap();
    let marked: Option<chrono::DateTime<Utc>> =
        sqlx::query_scalar("SELECT orphaned_at FROM attachments WHERE id = ?")
            .bind(attachment_id)
            .fetch_one(server.state.pool())
            .await
            .unwrap();
    assert!(marked.is_none());

    server
        .state
        .recall_message(
            Uuid::parse_str(&target_room).unwrap(),
            user.id,
            forwarded.id,
        )
        .await
        .unwrap();
    let (marked, storage_key): (Option<chrono::DateTime<Utc>>, String) =
        sqlx::query_as("SELECT orphaned_at, storage_key FROM attachments WHERE id = ?")
            .bind(attachment_id)
            .fetch_one(server.state.pool())
            .await
            .unwrap();
    assert!(marked.is_some());
    assert!(server
        .state
        .attachment_store()
        .exists(&storage_key)
        .await
        .unwrap());
}

#[tokio::test]
async fn restart_backfills_legacy_uuid_keyed_file_hash() {
    let root = std::env::temp_dir()
        .join("chat-room-hash-backfill")
        .join(Uuid::new_v4().simple().to_string());
    let database = root.join("rooms.db");
    let attachment_dir = root.join("attachments");
    let config = AppConfig {
        attachments: AttachmentConfig {
            directory: attachment_dir,
            ..AttachmentConfig::default()
        },
        ..AppConfig::default()
    };
    let state = AppState::open_with_config(&database, &config)
        .await
        .unwrap();
    let user = state.insert_user("legacy-owner", "not-used").await.unwrap();
    let room = Room {
        id: Uuid::new_v4(),
        name: "legacy-hash-room".into(),
        password_hash: String::new(),
        has_password: false,
        creator_user_id: Some(user.id),
        join_policy: "open".into(),
        avatar_emoji: String::new(),
        description: String::new(),
        membership_status: Some("active".into()),
        membership_role: Some("owner".into()),
        unread_count: 0,
        created_at: Utc::now(),
    };
    state
        .create_room_with_owner(room.clone(), user.id)
        .await
        .unwrap();
    let bytes = b"legacy-file-needs-backfill";
    let mut staged = state.attachment_store().begin().await.unwrap();
    staged.write(bytes).await.unwrap();
    let message = state
        .store_attachment_message(
            room.id,
            &user,
            &user.username,
            chat_room::message_store::NewAttachment {
                file_name: "legacy.bin".into(),
                mime_type: "application/octet-stream".into(),
                is_sensitive: false,
                staged,
            },
            "",
            None,
        )
        .await
        .unwrap();
    let attachment_id = message.attachment.unwrap().id;
    let storage_key: String =
        sqlx::query_scalar("SELECT storage_key FROM attachments WHERE id = ?")
            .bind(attachment_id)
            .fetch_one(state.pool())
            .await
            .unwrap();
    let legacy_key = attachment_id.simple().to_string();
    let legacy_path = state.attachment_store().path(&legacy_key);
    tokio::fs::create_dir_all(legacy_path.parent().unwrap())
        .await
        .unwrap();
    tokio::fs::rename(state.attachment_store().path(&storage_key), &legacy_path)
        .await
        .unwrap();
    sqlx::query("UPDATE attachments SET content_hash = NULL, storage_key = NULL WHERE id = ?")
        .bind(attachment_id)
        .execute(state.pool())
        .await
        .unwrap();
    drop(state);

    let reopened = AppState::open_with_config(&database, &config)
        .await
        .unwrap();
    let hash: String = sqlx::query_scalar("SELECT content_hash FROM attachments WHERE id = ?")
        .bind(attachment_id)
        .fetch_one(reopened.pool())
        .await
        .unwrap();
    assert_eq!(hash, hex::encode(Sha256::digest(bytes)));
    let key: Option<String> =
        sqlx::query_scalar("SELECT storage_key FROM attachments WHERE id = ?")
            .bind(attachment_id)
            .fetch_one(reopened.pool())
            .await
            .unwrap();
    assert!(key.is_none());
    let _ = tokio::fs::remove_dir_all(root).await;
}
