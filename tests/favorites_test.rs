use reqwest::{multipart, Client, StatusCode};
use uuid::Uuid;

#[allow(dead_code)]
mod favorites_support;
use favorites_support::{create_room, register, start_server};

#[tokio::test]
async fn files_can_be_uploaded_directly_to_favorites_and_forwarded() {
    let server = start_server().await;
    let client = Client::new();
    let (token, _) = register(&client, &server.base, "favorite-file-owner").await;
    let target_room = create_room(&client, &server.base, &token, "favorite-file-target").await;
    let bytes = b"favorite-image-bytes".to_vec();

    let response = client
        .post(format!("{}/api/favorites/attachments", server.base))
        .bearer_auth(&token)
        .multipart(
            multipart::Form::new()
                .text("title", "设计稿")
                .text("content", "## 评审要点\n\n- 检查间距")
                .part(
                    "file",
                    multipart::Part::bytes(bytes.clone())
                        .file_name("review.png")
                        .mime_str("image/png")
                        .unwrap(),
                ),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let favorite: serde_json::Value = response.json().await.unwrap();
    assert_eq!(favorite["kind"], "manual");
    assert_eq!(favorite["title"], "设计稿");
    assert_eq!(favorite["attachment"]["file_name"], "review.png");
    let attachment_id = Uuid::parse_str(favorite["attachment"]["id"].as_str().unwrap()).unwrap();
    let room_id: Option<Uuid> = sqlx::query_scalar("SELECT room_id FROM attachments WHERE id = ?")
        .bind(attachment_id)
        .fetch_one(server.state.pool())
        .await
        .unwrap();
    assert!(room_id.is_none());

    let downloaded = client
        .get(format!(
            "{}{}",
            server.base,
            favorite["attachment"]["download_url"].as_str().unwrap()
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(downloaded.status(), StatusCode::OK);
    assert_eq!(downloaded.bytes().await.unwrap().as_ref(), bytes);

    let forwarded: Vec<serde_json::Value> = client
        .post(format!(
            "{}/api/favorites/{}/forward",
            server.base,
            favorite["id"].as_str().unwrap()
        ))
        .bearer_auth(&token)
        .json(&serde_json::json!({ "target_room_ids": [target_room] }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(forwarded[0]["forwarded_message_id"].is_string());
    let messages: Vec<serde_json::Value> = client
        .get(format!("{}/api/rooms/{target_room}/messages", server.base))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(messages[0]["favorite_id"], favorite["id"]);
    assert_eq!(messages[0]["attachment"]["file_name"], "review.png");
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
    assert_eq!(forwarded[0]["favorite_id"], manual["id"]);

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
