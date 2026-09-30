//! TG-305: saved GIFs and GIF messages over HTTP — authorization identical to attachments
//! (only a GIF you can read can be saved or re-sent), recency order, the recent-GIFs list,
//! uploads, and the orphan sweep keeping saved GIFs alive. The same flow runs on PostgreSQL.

mod gif_support;
mod migration_support;
mod sticker_support;
mod support;

use std::sync::Arc;

use chat_room::config::{AdminConfig, AppConfig};
use chat_room::state::AppState;
use chrono::{Duration, Utc};
use gif_support::{
    delete, get_json, gif, join, mp4, save_gif, send_gif, upload_gif, valid_mp4, Mp4Spec,
};
use reqwest::StatusCode;
use serde_json::{json, Value};
use sticker_support::http::{self, Server};
use support::system_admin_token;
use uuid::Uuid;

fn id(value: &Value) -> String {
    value["id"].as_str().unwrap().to_string()
}

/// Upload, save, list, recent and send on any adapter.
async fn gif_flow(server: &Server, prefix: &str) {
    let alice = server.token(&format!("{prefix}-alice")).await;
    let bob = server.token(&format!("{prefix}-bob")).await;
    let mallory = server.token(&format!("{prefix}-mallory")).await;
    let group = server.create_chat(&alice, &format!("{prefix}-group")).await;
    join(server, &bob, &group).await;

    // Upload: a silent MP4 becomes a GIF message, broadcast as media_kind 'gif'.
    let (status, sent) = upload_gif(server, &alice, &group, valid_mp4()).await;
    assert_eq!(status, StatusCode::CREATED, "{sent}");
    assert_eq!(sent["media_kind"], "gif");
    assert_eq!(sent["attachment"]["mime_type"], "video/mp4");
    assert_eq!(sent["attachment"]["file_name"], "gif.mp4");
    let (status, rejected) = upload_gif(
        server,
        &alice,
        &group,
        mp4(Mp4Spec {
            audio: true,
            ..Mp4Spec::default()
        }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(rejected["error"], "audio_not_allowed");
    let (status, _) = upload_gif(server, &mallory, &group, valid_mp4()).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "a non-member cannot send");

    // A real GIF sent through the ordinary GIF upload is a GIF too.
    let (status, real) = upload_gif(server, &bob, &group, gif(200, 100)).await;
    assert_eq!(status, StatusCode::CREATED, "{real}");
    assert_eq!(real["attachment"]["mime_type"], "image/gif");

    // Recent GIFs: members see both, newest first, with the header geometry.
    let (status, recent) = get_json(server, &bob, "/api/gifs/recent").await;
    assert_eq!(status, StatusCode::OK);
    let recent = recent.as_array().unwrap();
    assert_eq!(recent.len(), 2, "{recent:?}");
    assert_eq!(recent[0]["message_id"], real["id"]);
    assert_eq!(
        (recent[0]["width"].as_i64(), recent[0]["height"].as_i64()),
        (Some(200), Some(100))
    );
    assert_eq!(recent[1]["message_id"], sent["id"]);
    assert_eq!(recent[1]["file_url"], sent["attachment"]["download_url"]);
    let (_, none) = get_json(server, &mallory, "/api/gifs/recent").await;
    assert_eq!(none, json!([]), "a non-member sees none of the chat's GIFs");

    // Save: only from a message the account can read.
    let (status, saved) = save_gif(server, &bob, &sent["id"]).await;
    assert_eq!(status, StatusCode::CREATED, "{saved}");
    assert_eq!(
        (saved["width"].as_i64(), saved["duration_ms"].as_i64()),
        (Some(480), Some(2_000))
    );
    let (status, _) = save_gif(server, &mallory, &sent["id"]).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "a non-member cannot save");
    let (status, _) = save_gif(server, &bob, &json!(Uuid::new_v4())).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = save_gif(server, &bob, &real["id"]).await;
    assert_eq!(status, StatusCode::CREATED);
    let (_, list) = get_json(server, &bob, "/api/gifs/saved").await;
    let order: Vec<&str> = list
        .as_array()
        .unwrap()
        .iter()
        .map(|g| g["mime_type"].as_str().unwrap())
        .collect();
    assert_eq!(order, ["image/gif", "video/mp4"], "newest first");
    // Re-saving moves it to the front and answers 200.
    tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    let (status, again) = save_gif(server, &bob, &sent["id"]).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(again["id"], saved["id"]);
    let (_, list) = get_json(server, &bob, "/api/gifs/saved").await;
    assert_eq!(list[0]["id"], saved["id"]);

    // Send by reference: a saved GIF moves to the front; another account's saved GIF 404s.
    let (status, resent) = send_gif(
        server,
        &bob,
        &group,
        json!({ "saved_gif_id": list[1]["id"], "client_message_id": Uuid::new_v4() }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{resent}");
    assert_eq!(resent["media_kind"], "gif");
    assert_eq!(resent["attachment"]["mime_type"], "image/gif");
    assert_ne!(resent["attachment"]["id"], real["attachment"]["id"]);
    let (_, list) = get_json(server, &bob, "/api/gifs/saved").await;
    assert_eq!(
        list[0]["mime_type"], "image/gif",
        "sending moves it to the front"
    );
    let (status, _) = send_gif(
        server,
        &alice,
        &group,
        json!({ "saved_gif_id": saved["id"] }),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = send_gif(server, &alice, &group, json!({ "message_id": sent["id"] })).await;
    assert_eq!(status, StatusCode::CREATED);
    let (status, _) = send_gif(server, &alice, &group, json!({})).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // The saved copy is served behind its own capability, byte ranges included.
    let file_url = saved["file_url"].as_str().unwrap();
    let (status, bytes) = server.fetch(file_url).await;
    assert_eq!((status, bytes), (StatusCode::OK, valid_mp4()));
    let ranged = server
        .client
        .get(server.url(file_url))
        .header("range", "bytes=4-7")
        .send()
        .await
        .unwrap();
    assert_eq!(ranged.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(ranged.bytes().await.unwrap().as_ref(), b"ftyp");
    let forged = format!("/api/gifs/saved/{}/file?key={}", id(&saved), Uuid::new_v4());
    assert_eq!(server.fetch(&forged).await.0, StatusCode::NOT_FOUND);

    // Leaving the chat revokes saving and the recent list, not what was already saved.
    assert_eq!(
        delete(server, &bob, &format!("/api/chats/{group}/members/me")).await,
        StatusCode::NO_CONTENT
    );
    let (status, _) = save_gif(server, &bob, &sent["id"]).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "a former member cannot save");
    let (_, recent) = get_json(server, &bob, "/api/gifs/recent").await;
    assert_eq!(recent, json!([]));
    let (_, list) = get_json(server, &bob, "/api/gifs/saved").await;
    assert_eq!(list.as_array().unwrap().len(), 2);

    // Remove: only one's own.
    let path = format!("/api/gifs/saved/{}", id(&saved));
    assert_eq!(delete(server, &alice, &path).await, StatusCode::NOT_FOUND);
    assert_eq!(delete(server, &bob, &path).await, StatusCode::NO_CONTENT);
    assert_eq!(server.fetch(file_url).await.0, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn gifs_are_saved_listed_and_sent_like_attachments_on_sqlite() {
    let server = http::start(AppConfig::default()).await;
    gif_flow(&server, "gif-sqlite").await;
}

#[tokio::test]
async fn gifs_are_saved_listed_and_sent_like_attachments_on_postgres() {
    let Some((admin_url, admin_pool)) = migration_support::postgres_admin_pool(
        "gifs_are_saved_listed_and_sent_like_attachments_on_postgres",
    )
    .await
    else {
        return;
    };
    let scratch = migration_support::create_postgres_scratch(&admin_url, &admin_pool, "gifs").await;
    let state = AppState::open_postgres(&scratch.url, &AppConfig::default())
        .await
        .unwrap();
    let server = http::start_with_state(Arc::new(state)).await;
    gif_flow(&server, "gif-pg").await;
    server.state.postgres_pool().unwrap().close().await;
    drop(server);
    migration_support::drop_postgres_scratch(&admin_pool, &scratch).await;
}

#[tokio::test]
async fn a_recalled_or_password_protected_gif_is_not_readable() {
    let server = http::start(AppConfig::default()).await;
    let alice = server.token("gif-recall-alice").await;
    let bob = server.token("gif-recall-bob").await;
    let direct = server.direct_chat(&alice, &bob).await;
    let (_, sent) = upload_gif(&server, &alice, &direct, gif(10, 10)).await;
    let message_id = Uuid::parse_str(&id(&sent)).unwrap();
    sqlx::query("UPDATE messages SET recalled_at = ? WHERE id = ?")
        .bind(Utc::now())
        .bind(message_id)
        .execute(server.state.pool())
        .await
        .unwrap();
    assert_eq!(
        save_gif(&server, &bob, &sent["id"]).await.0,
        StatusCode::NOT_FOUND
    );
    let (status, _) = send_gif(&server, &bob, &direct, json!({ "message_id": sent["id"] })).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(
        get_json(&server, &bob, "/api/gifs/recent").await.1,
        json!([])
    );

    // A plain photo is readable but is not a GIF.
    let (_, photo) = upload_gif(&server, &alice, &direct, gif(10, 10)).await;
    sqlx::query("UPDATE messages SET media_kind = NULL WHERE id = ?")
        .bind(Uuid::parse_str(&id(&photo)).unwrap())
        .execute(server.state.pool())
        .await
        .unwrap();
    sqlx::query("UPDATE attachments SET mime_type = 'image/png' WHERE id = ?")
        .bind(Uuid::parse_str(photo["attachment"]["id"].as_str().unwrap()).unwrap())
        .execute(server.state.pool())
        .await
        .unwrap();
    let (status, body) = save_gif(&server, &bob, &photo["id"]).await;
    assert_eq!(
        (status, body["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("not_a_gif"))
    );

    // A password chat's GIFs never appear in the recent list (history needs the password).
    let locked = server.create_chat(&alice, "gif-locked").await;
    let (_, locked_gif) = upload_gif(&server, &alice, &locked, gif(12, 12)).await;
    sqlx::query("UPDATE chats SET password_hash = 'x' WHERE id = ?")
        .bind(Uuid::parse_str(&locked).unwrap())
        .execute(server.state.pool())
        .await
        .unwrap();
    let (_, recent) = get_json(&server, &alice, "/api/gifs/recent").await;
    assert!(
        recent
            .as_array()
            .unwrap()
            .iter()
            .all(|g| g["message_id"] != locked_gif["id"]),
        "{recent}"
    );
}

#[tokio::test]
async fn the_orphan_sweep_keeps_a_saved_gif_until_it_is_removed() {
    let config = AppConfig {
        admin: AdminConfig {
            usernames: vec!["gif-ops".into()],
            orphan_retention_hours: 1,
            deleted_room_retention_days: 1,
        },
        ..AppConfig::default()
    };
    let server = http::start(config).await;
    let alice = server.token("gif-sweep-alice").await;
    let bob = server.token("gif-sweep-bob").await;
    let direct = server.direct_chat(&alice, &bob).await;
    let (_, sent) = upload_gif(&server, &alice, &direct, gif(33, 44)).await;
    let (status, saved) = save_gif(&server, &bob, &sent["id"]).await;
    assert_eq!(status, StatusCode::CREATED);

    // The message is recalled and its attachment rows are past the retention window.
    let pool = server.state.pool();
    let two_hours_ago = Utc::now() - Duration::hours(2);
    sqlx::query("UPDATE messages SET recalled_at = ? WHERE id = ?")
        .bind(two_hours_ago)
        .bind(Uuid::parse_str(&id(&sent)).unwrap())
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("UPDATE attachments SET orphaned_at = ?")
        .bind(two_hours_ago)
        .execute(pool)
        .await
        .unwrap();
    let admin = system_admin_token(&server.state, &server.base, "gif-ops").await;
    let purge = || async {
        server
            .client
            .post(server.url("/api/admin/maintenance/purge"))
            .bearer_auth(&admin)
            .send()
            .await
            .unwrap()
            .json::<Value>()
            .await
            .unwrap()
    };
    let result = purge().await;
    assert_eq!(result["attachment_objects_deleted"], 0, "{result}");
    let file_url = saved["file_url"].as_str().unwrap();
    assert_eq!(server.fetch(file_url).await, (StatusCode::OK, gif(33, 44)));

    // Removing the last saved reference hands the object back to the sweep.
    let path = format!("/api/gifs/saved/{}", id(&saved));
    assert_eq!(delete(&server, &bob, &path).await, StatusCode::NO_CONTENT);
    let result = purge().await;
    assert_eq!(result["attachment_objects_deleted"], 1, "{result}");
}
