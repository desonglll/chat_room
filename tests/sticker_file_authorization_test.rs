//! TG-302: a sticker sent into a private chat is authorized exactly like an attachment, and
//! the orphan sweep never deletes an object a live sticker still serves.

mod sticker_support;
mod support;

use chat_room::config::{AdminConfig, AppConfig};
use chrono::{Duration, Utc};
use reqwest::StatusCode;
use serde_json::{json, Value};
use sticker_support::{http, valid_tgs};
use support::system_admin_token;
use uuid::Uuid;

#[tokio::test]
async fn sticker_files_in_a_private_chat_are_authorized_like_attachments() {
    let config = AppConfig {
        admin: AdminConfig {
            usernames: vec!["sticker-ops".into()],
            orphan_retention_hours: 1,
            deleted_room_retention_days: 1,
        },
        ..AppConfig::default()
    };
    let server = http::start(config).await;
    let (alice, sticker) = server
        .seeded_sticker("sticker-private-alice", "private_pack")
        .await;
    let bob = server.token("sticker-private-bob").await;
    let mallory = server.token("sticker-private-mallory").await;
    let direct = server.direct_chat(&alice, &bob).await;

    let (status, sent) = server
        .send_sticker(&alice, &direct, json!({ "sticker_id": sticker["id"] }))
        .await;
    assert_eq!(status, StatusCode::CREATED, "{sent}");
    let download_url = sent["attachment"]["download_url"]
        .as_str()
        .unwrap()
        .to_string();
    let attachment_id = sent["attachment"]["id"].as_str().unwrap().to_string();

    // The member sees and fetches it through the ordinary attachment capability.
    let (status, history) = server.history(&bob, &direct).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        history.as_array().unwrap().last().unwrap()["attachment"]["download_url"],
        download_url
    );
    assert_eq!(server.fetch(&download_url).await.0, StatusCode::OK);

    // A non-member cannot read the history that carries the capability, cannot send into
    // the chat, and cannot turn the public catalogue key into the message's file.
    assert_eq!(
        server.history(&mallory, &direct).await.0,
        StatusCode::FORBIDDEN
    );
    let (status, _) = server
        .send_sticker(&mallory, &direct, json!({ "sticker_id": sticker["id"] }))
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let catalogue_key = sticker["file_url"]
        .as_str()
        .unwrap()
        .rsplit_once("key=")
        .unwrap()
        .1;
    let forged = format!("/api/attachments/{attachment_id}?key={catalogue_key}");
    assert_eq!(server.fetch(&forged).await.0, StatusCode::NOT_FOUND);
    assert_eq!(
        server
            .fetch(&format!(
                "/api/attachments/{attachment_id}?key={}",
                Uuid::new_v4()
            ))
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    let wrong_catalogue = format!(
        "/api/stickers/{}/file?key={}",
        sticker["id"].as_str().unwrap(),
        Uuid::new_v4()
    );
    assert_eq!(
        server.fetch(&wrong_catalogue).await.0,
        StatusCode::NOT_FOUND
    );

    // Recall: like any attachment the message copy dies, and the viewer loses the sticker.
    let pool = server.state.pool();
    let message_id = Uuid::parse_str(sent["id"].as_str().unwrap()).unwrap();
    let attachment_uuid = Uuid::parse_str(&attachment_id).unwrap();
    sqlx::query("UPDATE messages SET recalled_at = ? WHERE id = ?")
        .bind(Utc::now() - Duration::hours(2))
        .bind(message_id)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("UPDATE attachments SET orphaned_at = ? WHERE id = ?")
        .bind(Utc::now() - Duration::hours(2))
        .bind(attachment_uuid)
        .execute(pool)
        .await
        .unwrap();
    assert_eq!(server.fetch(&download_url).await.0, StatusCode::NOT_FOUND);
    let (_, history) = server.history(&bob, &direct).await;
    let recalled = history.as_array().unwrap().last().unwrap();
    assert!(recalled.get("sticker").is_none() && recalled["attachment"].is_null());

    // The orphan sweep must not delete an object a live sticker still serves.
    let admin = system_admin_token(&server.state, &server.base, "sticker-ops").await;
    let purge: Value = server
        .client
        .post(server.url("/api/admin/maintenance/purge"))
        .bearer_auth(&admin)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(purge["attachment_objects_deleted"], 0, "{purge}");
    let (status, file) = server.fetch(sticker["file_url"].as_str().unwrap()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(file, valid_tgs());
}
