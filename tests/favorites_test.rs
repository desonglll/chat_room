use std::sync::Arc;

use chat_room::{build_app, state::AppState};
use reqwest::{multipart, Client, StatusCode};
use tokio::net::TcpListener;
use uuid::Uuid;

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

async fn start_server() -> TestServer {
    let state = Arc::new(AppState::new().await.unwrap());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn({
        let state = state.clone();
        async move { axum::serve(listener, build_app(state)).await.unwrap() }
    });
    TestServer {
        base: format!("http://{address}"),
        state,
        task,
    }
}

async fn register(client: &Client, base: &str, username: &str) -> (String, Uuid) {
    let session: serde_json::Value = client
        .post(format!("{base}/api/users/register"))
        .json(&serde_json::json!({
            "username": username,
            "password": "test-password"
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    (
        session["token"].as_str().unwrap().to_owned(),
        Uuid::parse_str(session["user"]["id"].as_str().unwrap()).unwrap(),
    )
}

async fn create_room(client: &Client, base: &str, token: &str, name: &str) -> Uuid {
    let room: serde_json::Value = client
        .post(format!("{base}/api/rooms"))
        .bearer_auth(token)
        .json(&serde_json::json!({ "name": name, "password": "", "join_policy": "open" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    Uuid::parse_str(room["id"].as_str().unwrap()).unwrap()
}

#[tokio::test]
async fn favorites_preserve_video_support_manual_items_and_forwarding() {
    let server = start_server().await;
    let client = Client::new();
    let (token, user_id) = register(&client, &server.base, "favorite-user").await;
    let source_room = create_room(&client, &server.base, &token, "favorite-source").await;
    let target_room = create_room(&client, &server.base, &token, "favorite-target").await;
    let (outsider_token, _) = register(&client, &server.base, "favorite-outsider").await;
    let outsider_room = create_room(
        &client,
        &server.base,
        &outsider_token,
        "favorite-private-target",
    )
    .await;

    let video_bytes = b"test-video-content".to_vec();
    let upload: serde_json::Value = client
        .post(format!(
            "{}/api/rooms/{source_room}/attachments",
            server.base
        ))
        .bearer_auth(&token)
        .multipart(
            multipart::Form::new()
                .part(
                    "file",
                    multipart::Part::bytes(video_bytes.clone())
                        .file_name("clip.mp4")
                        .mime_str("video/mp4")
                        .unwrap(),
                )
                .text("content", "值得保存的视频"),
        )
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let message_id = Uuid::parse_str(upload["id"].as_str().unwrap()).unwrap();
    let download_url = upload["attachment"]["download_url"]
        .as_str()
        .unwrap()
        .to_owned();

    let favorites: Vec<serde_json::Value> = client
        .post(format!("{}/api/favorites/messages", server.base))
        .bearer_auth(&token)
        .json(&serde_json::json!({ "message_ids": [message_id] }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(favorites.len(), 1);
    assert_eq!(favorites[0]["kind"], "video");
    assert_eq!(favorites[0]["content"], "值得保存的视频");
    assert_eq!(favorites[0]["source_room_id"], source_room.to_string());
    let video_favorite_id = favorites[0]["id"].as_str().unwrap();

    let duplicate: Vec<serde_json::Value> = client
        .post(format!("{}/api/favorites/messages", server.base))
        .bearer_auth(&token)
        .json(&serde_json::json!({ "message_ids": [message_id] }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(duplicate[0]["id"], favorites[0]["id"]);

    assert!(server
        .state
        .recall_message(source_room, user_id, message_id)
        .await
        .unwrap()
        .is_some());
    let preserved = client
        .get(format!("{}{}", server.base, download_url))
        .send()
        .await
        .unwrap();
    assert_eq!(preserved.status(), StatusCode::OK);
    assert_eq!(preserved.bytes().await.unwrap().as_ref(), video_bytes);

    let recalled_favorites: Vec<serde_json::Value> = client
        .get(format!("{}/api/favorites", server.base))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        recalled_favorites[0]["source_room_id"],
        source_room.to_string()
    );

    let manual: serde_json::Value = client
        .post(format!("{}/api/favorites", server.base))
        .bearer_auth(&token)
        .json(&serde_json::json!({
            "title": "待办",
            "content": "稍后整理会议结论"
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(manual["kind"], "manual");

    let outcomes: Vec<serde_json::Value> = client
        .post(format!(
            "{}/api/favorites/{}/forward",
            server.base,
            manual["id"].as_str().unwrap()
        ))
        .bearer_auth(&token)
        .json(&serde_json::json!({ "target_room_ids": [target_room, outsider_room] }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(outcomes[0]["forwarded_message_id"].is_string());
    assert_eq!(
        outcomes[1]["skipped_reason"],
        "cannot send to the target room"
    );

    let forwarded: Vec<serde_json::Value> = client
        .get(format!("{}/api/rooms/{target_room}/messages", server.base))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(forwarded[0]["content"], "稍后整理会议结论");
    assert_eq!(forwarded[0]["forwarded_from"]["sender"], "我的收藏");

    assert_eq!(
        client
            .delete(format!("{}/api/favorites/{video_favorite_id}", server.base))
            .bearer_auth(&token)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::NO_CONTENT
    );
    let remaining: Vec<serde_json::Value> = client
        .get(format!("{}/api/favorites", server.base))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0]["id"], manual["id"]);
}
