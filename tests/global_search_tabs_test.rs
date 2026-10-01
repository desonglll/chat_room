//! TG-504 search tabs: each kind of result (media, files, links, music, voice) is its own
//! filter with its own cursor, file names match as well as text, and results are authorized at
//! read time — a chat the viewer has left drops out even though its messages still match.

use chrono::{DateTime, Duration, Utc};
use reqwest::{Client, StatusCode};
use serde_json::Value;
use uuid::Uuid;

#[allow(dead_code)]
mod global_search_support;
use global_search_support::{create_chat, insert_message, register, search, start_server};

async fn attach(
    state: &chat_room::state::AppState,
    room: Uuid,
    uploader: Uuid,
    name: &str,
    mime: &str,
) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO attachments \
         (id, access_key, room_id, uploader_id, file_name, mime_type, size_bytes, created_at) \
         VALUES (?, ?, ?, ?, ?, ?, 12, ?)",
    )
    .bind(id)
    .bind(Uuid::new_v4())
    .bind(room)
    .bind(uploader)
    .bind(name)
    .bind(mime)
    .bind(Utc::now())
    .execute(state.pool())
    .await
    .unwrap();
    id
}

fn ids(page: &Value) -> Vec<String> {
    page["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["excerpt"].as_str().unwrap().to_string())
        .collect()
}

#[tokio::test]
async fn search_tabs_filter_by_kind_paginate_alone_and_authorize_at_read_time() {
    let server = start_server().await;
    let client = Client::new();
    let viewer = register(&client, &server.base, "tabs-viewer").await;
    let chat = create_chat(&client, &server.base, &viewer, "Tabs").await;
    let state = &server.state;
    let at = |seconds: i64| {
        DateTime::parse_from_rfc3339("2026-09-01T08:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
            + Duration::seconds(seconds)
    };

    let photo = attach(state, chat, viewer.id, "zebra-photo.png", "image/png").await;
    let clip = attach(state, chat, viewer.id, "zebra-clip.mp4", "video/mp4").await;
    let doc = attach(
        state,
        chat,
        viewer.id,
        "zebra-report.pdf",
        "application/pdf",
    )
    .await;
    let song = attach(state, chat, viewer.id, "zebra-song.mp3", "audio/mpeg").await;
    let memo = attach(state, chat, viewer.id, "voice.ogg", "audio/ogg").await;
    insert_message(
        state,
        Uuid::new_v4(),
        chat,
        &viewer,
        "zebra text",
        None,
        at(1),
    )
    .await;
    insert_message(
        state,
        Uuid::new_v4(),
        chat,
        &viewer,
        "zebra see https://example.com",
        None,
        at(2),
    )
    .await;
    insert_message(
        state,
        Uuid::new_v4(),
        chat,
        &viewer,
        "photo of zebra",
        Some(photo),
        at(3),
    )
    .await;
    insert_message(
        state,
        Uuid::new_v4(),
        chat,
        &viewer,
        "clip",
        Some(clip),
        at(4),
    )
    .await;
    insert_message(state, Uuid::new_v4(), chat, &viewer, "", Some(doc), at(5)).await;
    insert_message(state, Uuid::new_v4(), chat, &viewer, "", Some(song), at(6)).await;
    let voice_id = Uuid::new_v4();
    insert_message(
        state,
        voice_id,
        chat,
        &viewer,
        "zebra memo",
        Some(memo),
        at(7),
    )
    .await;
    sqlx::query("UPDATE messages SET media_kind = 'voice' WHERE id = ?")
        .bind(voice_id)
        .execute(state.pool())
        .await
        .unwrap();

    let tab = |kind: &'static str| {
        let client = client.clone();
        let base = server.base.clone();
        let token = viewer.token.clone();
        async move {
            search(
                &client,
                &base,
                &token,
                &format!("q=zebra&content_type={kind}"),
            )
            .await
        }
    };
    assert_eq!(
        ids(&tab("media").await),
        ["clip", "photo of zebra"],
        "file names match too"
    );
    assert_eq!(ids(&tab("document").await), [""]);
    assert_eq!(
        tab("document").await["items"][0]["attachment_file_name"],
        "zebra-report.pdf"
    );
    assert_eq!(ids(&tab("link").await), ["zebra see https://example.com"]);
    assert_eq!(
        tab("music").await["items"][0]["attachment_file_name"],
        "zebra-song.mp3"
    );
    assert_eq!(ids(&tab("voice").await), ["zebra memo"]);
    assert_eq!(tab("voice").await["items"][0]["content_type"], "voice");
    assert_eq!(tab("all").await["items"].as_array().unwrap().len(), 7);

    // Each tab pages on its own cursor.
    let first = search(
        &client,
        &server.base,
        &viewer.token,
        "q=zebra&content_type=media&limit=1",
    )
    .await;
    assert_eq!(ids(&first), ["clip"]);
    let cursor = first["next_cursor"].as_str().unwrap();
    let second = client
        .get(format!("{}/api/messages/search", server.base))
        .bearer_auth(&viewer.token)
        .query(&[
            ("q", "zebra"),
            ("content_type", "media"),
            ("limit", "1"),
            ("cursor", cursor),
        ])
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    assert_eq!(ids(&second), ["photo of zebra"]);
    assert!(second["next_cursor"].is_null());

    // Read-time authorization: a member who leaves stops seeing what is already indexed there.
    let member = register(&client, &server.base, "tabs-member").await;
    let joined = client
        .post(format!("{}/api/chats/{chat}/join-requests", server.base))
        .bearer_auth(&member.token)
        .json(&serde_json::json!({}))
        .send()
        .await
        .unwrap()
        .status();
    assert_eq!(joined, StatusCode::OK);
    let member_search = || search(&client, &server.base, &member.token, "q=zebra");
    assert_eq!(member_search().await["items"].as_array().unwrap().len(), 7);
    let left = client
        .delete(format!("{}/api/chats/{chat}/members/me", server.base))
        .bearer_auth(&member.token)
        .send()
        .await
        .unwrap()
        .status();
    assert!(left.is_success(), "{left}");
    assert!(member_search().await["items"]
        .as_array()
        .unwrap()
        .is_empty());
}
