use std::sync::Arc;

use chat_room::{build_app, state::AppState};
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

async fn create_session(
    base: &str,
    room_id: &str,
    token: &str,
    name: &str,
    size: usize,
    fingerprint: &str,
    content_hash: Option<&str>,
) -> serde_json::Value {
    let response = reqwest::Client::new()
        .post(format!("{base}/api/rooms/{room_id}/attachments/uploads"))
        .bearer_auth(token)
        .json(&serde_json::json!({
            "file_name": name,
            "mime_type": "application/octet-stream",
            "size_bytes": size,
            "fingerprint": fingerprint,
            "content_hash": content_hash,
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    response.json().await.unwrap()
}

async fn send_chunk(
    base: &str,
    token: &str,
    upload_id: &str,
    offset: usize,
    bytes: &[u8],
) -> reqwest::Response {
    reqwest::Client::new()
        .put(format!(
            "{base}/api/attachments/uploads/{upload_id}/chunks?offset={offset}"
        ))
        .bearer_auth(token)
        .body(bytes.to_vec())
        .send()
        .await
        .unwrap()
}

async fn complete(base: &str, token: &str, upload_id: &str) -> serde_json::Value {
    let response = reqwest::Client::new()
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
async fn interrupted_upload_is_listed_and_resumes_from_confirmed_offset() {
    let server = start().await;
    let (room_id, token) = room_for(&server.base, "resume-owner", "resume-room").await;
    let bytes = b"resumable-content-across-a-browser-refresh";
    let hash = hex::encode(Sha256::digest(bytes));
    let fingerprint = "resume.bin:41:123";
    let first = create_session(
        &server.base,
        &room_id,
        &token,
        "resume.bin",
        bytes.len(),
        fingerprint,
        None,
    )
    .await;
    let upload_id = first["upload_id"].as_str().unwrap();
    assert_eq!(
        send_chunk(&server.base, &token, upload_id, 0, &bytes[..11])
            .await
            .status(),
        200
    );

    let resumed = create_session(
        &server.base,
        &room_id,
        &token,
        "resume.bin",
        bytes.len(),
        fingerprint,
        Some(&hash),
    )
    .await;
    assert_eq!(resumed["upload_id"], upload_id);
    assert_eq!(resumed["received_bytes"], 11);
    assert_eq!(resumed["deduplicated"], false);

    let wrong_hash = "0".repeat(64);
    let mismatch = reqwest::Client::new()
        .post(format!(
            "{}/api/rooms/{room_id}/attachments/uploads",
            server.base
        ))
        .bearer_auth(&token)
        .json(&serde_json::json!({
            "file_name": "resume.bin",
            "mime_type": "application/octet-stream",
            "size_bytes": bytes.len(),
            "fingerprint": fingerprint,
            "content_hash": wrong_hash,
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(mismatch.status(), 409);

    let listed: serde_json::Value = reqwest::Client::new()
        .get(format!(
            "{}/api/rooms/{room_id}/attachments/uploads",
            server.base
        ))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(listed[0]["id"], upload_id);
    assert_eq!(listed[0]["received_bytes"], 11);

    let conflict = send_chunk(&server.base, &token, upload_id, 0, b"wrong-offset").await;
    assert_eq!(conflict.status(), 409);
    assert_eq!(
        conflict.json::<serde_json::Value>().await.unwrap()["received_bytes"],
        11
    );
    assert_eq!(
        send_chunk(&server.base, &token, upload_id, 11, &bytes[11..])
            .await
            .status(),
        200
    );
    complete(&server.base, &token, upload_id).await;
}

#[tokio::test]
async fn repeat_upload_by_same_user_reuses_content_without_receiving_chunks() {
    let server = start().await;
    let (room_id, token) = room_for(&server.base, "dedupe-owner", "dedupe-room").await;
    let bytes = b"already-uploaded-content";
    let hash = hex::encode(Sha256::digest(bytes));
    let first = create_session(
        &server.base,
        &room_id,
        &token,
        "first.bin",
        bytes.len(),
        "first-upload",
        Some(&hash),
    )
    .await;
    let first_id = first["upload_id"].as_str().unwrap();
    assert_eq!(
        send_chunk(&server.base, &token, first_id, 0, bytes)
            .await
            .status(),
        200
    );
    complete(&server.base, &token, first_id).await;

    let second = create_session(
        &server.base,
        &room_id,
        &token,
        "second.bin",
        bytes.len(),
        "second-upload",
        Some(&hash),
    )
    .await;
    assert_eq!(second["received_bytes"], bytes.len());
    assert_eq!(second["deduplicated"], true);
    let second_id = second["upload_id"].as_str().unwrap();
    assert_eq!(
        server
            .state
            .attachment_store()
            .chunked_upload_size(Uuid::parse_str(second_id).unwrap())
            .await
            .unwrap(),
        0
    );
    complete(&server.base, &token, second_id).await;

    let counts: (i64, i64) = sqlx::query_as(
        "SELECT COUNT(*), COUNT(DISTINCT storage_key) FROM attachments WHERE room_id = ?",
    )
    .bind(Uuid::parse_str(&room_id).unwrap())
    .fetch_one(server.state.pool())
    .await
    .unwrap();
    assert_eq!(counts, (2, 1));

    let (other_room, other_token) = room_for(&server.base, "other-owner", "other-room").await;
    let foreign = create_session(
        &server.base,
        &other_room,
        &other_token,
        "guessed.bin",
        bytes.len(),
        "foreign-upload",
        Some(&hash),
    )
    .await;
    assert_eq!(foreign["received_bytes"], 0);
    assert_eq!(foreign["deduplicated"], false);
}
