use std::sync::Arc;

use chat_room::{build_app, state::AppState};
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
    let state = Arc::new(AppState::new().await.unwrap());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn({
        let state = state.clone();
        async move { axum::serve(listener, build_app(state)).await.unwrap() }
    });
    Server { base, state, task }
}

async fn upload(base: &str, room: &str, token: &str, name: &str, mime: &str) -> serde_json::Value {
    let part = multipart::Part::bytes(format!("bytes-{name}").into_bytes())
        .file_name(name.to_string())
        .mime_str(mime)
        .unwrap();
    reqwest::Client::new()
        .post(format!("{base}/api/rooms/{room}/attachments"))
        .bearer_auth(token)
        .multipart(multipart::Form::new().part("file", part))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap()
}

#[tokio::test]
async fn room_files_are_disk_backed_filtered_and_paginated() {
    let server = start().await;
    let token = session_token(&server.base, "files-owner").await;
    let room: serde_json::Value = reqwest::Client::new()
        .post(format!("{}/api/rooms", server.base))
        .bearer_auth(&token)
        .json(&serde_json::json!({ "name": "file-pages", "password": "" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let room_id = room["id"].as_str().unwrap();
    let image = upload(&server.base, room_id, &token, "one.png", "image/png").await;
    let video = upload(&server.base, room_id, &token, "two.mp4", "video/mp4").await;
    let document = upload(
        &server.base,
        room_id,
        &token,
        "three.pdf",
        "application/pdf",
    )
    .await;

    let first: serde_json::Value = reqwest::Client::new()
        .get(format!(
            "{}/api/rooms/{room_id}/files?limit=2&kind=all",
            server.base
        ))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(first["items"].as_array().unwrap().len(), 2);
    let cursor = first["next_before"].as_str().unwrap();
    let second: serde_json::Value = reqwest::Client::new()
        .get(format!(
            "{}/api/rooms/{room_id}/files?limit=2&kind=all&before={cursor}",
            server.base
        ))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(second["items"].as_array().unwrap().len(), 1);

    let images: serde_json::Value = reqwest::Client::new()
        .get(format!(
            "{}/api/rooms/{room_id}/files?kind=image",
            server.base
        ))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(images["items"][0]["attachment"]["file_name"], "one.png");

    let attachment_id = Uuid::parse_str(document["attachment"]["id"].as_str().unwrap()).unwrap();
    let storage_key: Option<String> =
        sqlx::query_scalar("SELECT storage_key FROM attachments WHERE id = ?")
            .bind(attachment_id)
            .fetch_one(server.state.pool())
            .await
            .unwrap();
    let storage_key = storage_key.unwrap_or_else(|| attachment_id.simple().to_string());
    assert!(
        tokio::fs::metadata(server.state.attachment_store().path(&storage_key))
            .await
            .is_ok()
    );
    let has_blob: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM pragma_table_info('attachments') WHERE name = 'data')",
    )
    .fetch_one(server.state.pool())
    .await
    .unwrap();
    assert!(!has_blob);

    let video_id = Uuid::parse_str(video["id"].as_str().unwrap()).unwrap();
    sqlx::query("UPDATE messages SET recalled_at = CURRENT_TIMESTAMP WHERE id = ?")
        .bind(video_id)
        .execute(server.state.pool())
        .await
        .unwrap();
    let videos: serde_json::Value = reqwest::Client::new()
        .get(format!(
            "{}/api/rooms/{room_id}/files?kind=video",
            server.base
        ))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(videos["items"].as_array().unwrap().is_empty());
    assert!(image["attachment"].is_object());
}
