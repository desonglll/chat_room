//! TG-304: resolving custom emoji, the installed custom emoji library, and emoji status with
//! its read-time authorization and expiry.

mod custom_emoji_support;
mod migration_support;
mod sticker_support;

use chrono::{Duration, Utc};
use custom_emoji_support::{on_postgres, on_sqlite};
use reqwest::StatusCode;
use serde_json::{json, Value};
use sticker_support::{http, valid_tgs, webp_lossless};
use uuid::Uuid;

async fn get(server: &http::Server, token: &str, path: &str) -> (StatusCode, Value) {
    let response = server
        .client
        .get(server.url(path))
        .bearer_auth(token)
        .send()
        .await
        .unwrap();
    let status = response.status();
    (status, response.json().await.unwrap_or(Value::Null))
}

async fn put_status(server: &http::Server, token: &str, body: Value) -> (StatusCode, Value) {
    let response = server
        .client
        .put(server.url("/api/users/me/emoji-status"))
        .bearer_auth(token)
        .json(&body)
        .send()
        .await
        .unwrap();
    let status = response.status();
    (status, response.json().await.unwrap_or(Value::Null))
}

async fn install(server: &http::Server, token: &str, set_id: &Value) {
    let response = server
        .client
        .put(server.url(&format!(
            "/api/stickers/installed/{}",
            set_id.as_str().unwrap()
        )))
        .bearer_auth(token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn custom_emoji_resolve_and_installed_library_sqlite() {
    on_sqlite(resolve_and_library).await;
}

#[tokio::test]
async fn custom_emoji_resolve_and_installed_library_postgres() {
    on_postgres(
        "custom_emoji_resolve_and_installed_library_postgres",
        resolve_and_library,
    )
    .await;
}

async fn resolve_and_library(server: http::Server) {
    let owner = server.token("ce-owner").await;
    let emoji_set = server.create_set(&owner, "ce_cats", "custom_emoji").await;
    let (status, emoji) = server
        .upload(&owner, "ce_cats", webp_lossless(100, 100), "😺 😸")
        .await;
    assert_eq!(status, StatusCode::CREATED, "{emoji}");
    let (status, rejected) = server
        .upload(&owner, "ce_cats", webp_lossless(512, 512), "😺")
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "custom emoji are 100×100");
    assert_eq!(rejected["error"], "invalid_dimensions");
    let regular_set = server.create_set(&owner, "ce_regular", "regular").await;
    let (_, sticker) = server.upload(&owner, "ce_regular", valid_tgs(), "🦀").await;

    // Any signed-in account resolves a custom emoji (no install needed); ordinary stickers
    // and unknown ids are simply absent.
    let viewer = server.token("ce-viewer").await;
    let ids = format!(
        "{},{},{}",
        sticker["id"].as_str().unwrap(),
        Uuid::new_v4(),
        emoji["id"].as_str().unwrap()
    );
    let (status, resolved) = get(&server, &viewer, &format!("/api/custom-emoji?ids={ids}")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(resolved.as_array().unwrap().len(), 1);
    let resolved = &resolved[0];
    assert_eq!(resolved["id"], emoji["id"]);
    assert_eq!(resolved["emoji"], "😺");
    assert_eq!(resolved["set_short_name"], "ce_cats");
    assert_eq!(resolved["format"], "webp");
    assert_eq!(
        (resolved["width"].as_i64(), resolved["height"].as_i64()),
        (Some(100), Some(100))
    );
    let (status, file) = server.fetch(resolved["file_url"].as_str().unwrap()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(file, webp_lossless(100, 100));

    let (status, _) = get(&server, &viewer, "/api/custom-emoji?ids=not-a-uuid").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let too_many = (0..201)
        .map(|_| Uuid::new_v4().to_string())
        .collect::<Vec<_>>()
        .join(",");
    let (status, body) = get(
        &server,
        &viewer,
        &format!("/api/custom-emoji?ids={too_many}"),
    )
    .await;
    assert_eq!(
        (status, body["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("too_many_ids"))
    );
    let unauthenticated = server
        .client
        .get(server.url("/api/custom-emoji?ids="))
        .send()
        .await
        .unwrap();
    assert_eq!(unauthenticated.status(), StatusCode::UNAUTHORIZED);

    // The installed custom emoji library is the sticker library filtered by set type.
    install(&server, &viewer, &regular_set["id"]).await;
    install(&server, &viewer, &emoji_set["id"]).await;
    let (status, library) = get(&server, &viewer, "/api/custom-emoji/installed").await;
    assert_eq!(status, StatusCode::OK);
    let sets = library["sets"].as_array().unwrap();
    assert_eq!(sets.len(), 1);
    assert_eq!(sets[0]["short_name"], "ce_cats");
    assert_eq!(sets[0]["stickers"][0]["id"], emoji["id"]);

    // A removed custom emoji no longer resolves: clients fall back to the Unicode emoji.
    let removed = server
        .client
        .delete(server.url(&format!(
            "/api/sticker-sets/ce_cats/stickers/{}",
            emoji["id"].as_str().unwrap()
        )))
        .bearer_auth(&owner)
        .send()
        .await
        .unwrap();
    assert_eq!(removed.status(), StatusCode::NO_CONTENT);
    let (_, resolved) = get(
        &server,
        &viewer,
        &format!("/api/custom-emoji?ids={}", emoji["id"].as_str().unwrap()),
    )
    .await;
    assert_eq!(resolved, json!([]));
}

#[tokio::test]
async fn emoji_status_is_visible_only_to_chat_peers_and_expires_sqlite() {
    on_sqlite(emoji_status_flow).await;
}

#[tokio::test]
async fn emoji_status_is_visible_only_to_chat_peers_and_expires_postgres() {
    on_postgres(
        "emoji_status_is_visible_only_to_chat_peers_and_expires_postgres",
        emoji_status_flow,
    )
    .await;
}

async fn emoji_status_flow(server: http::Server) {
    let alice = server.token("es-alice").await;
    server.create_set(&alice, "es_pack", "custom_emoji").await;
    let (_, emoji) = server
        .upload(&alice, "es_pack", webp_lossless(100, 100), "⭐")
        .await;
    server.create_set(&alice, "es_regular", "regular").await;
    let (_, sticker) = server.upload(&alice, "es_regular", valid_tgs(), "🦀").await;
    let bob = server.token("es-bob").await;
    let stranger = server.token("es-stranger").await;
    let alice_id = server.user_id(&alice).await;
    let path = format!("/api/users/emoji-statuses?ids={alice_id}");

    let expires_at = Utc::now() + Duration::hours(1);
    let (status, set) = put_status(
        &server,
        &alice,
        json!({ "custom_emoji_id": emoji["id"], "expires_at": expires_at }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{set}");
    assert_eq!(set["user_id"], alice_id);
    assert_eq!(set["emoji"]["emoji"], "⭐");

    let (status, _) =
        put_status(&server, &alice, json!({ "custom_emoji_id": sticker["id"] })).await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "a regular sticker is not a custom emoji"
    );
    let past = Utc::now() - Duration::minutes(1);
    let (status, body) = put_status(
        &server,
        &alice,
        json!({ "custom_emoji_id": emoji["id"], "expires_at": past }),
    )
    .await;
    assert_eq!(
        (status, body["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("invalid_expiry"))
    );

    // Visible to herself, not to an account she shares no chat with, then to bob once they do.
    let (_, own) = get(&server, &alice, &path).await;
    assert_eq!(own[0]["custom_emoji_id"], emoji["id"]);
    let (_, hidden) = get(&server, &stranger, &path).await;
    assert_eq!(hidden, json!([]));
    let (_, hidden) = get(&server, &bob, &path).await;
    assert_eq!(hidden, json!([]));
    server.direct_chat(&alice, &bob).await;
    let (status, visible) = get(&server, &bob, &path).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(visible[0]["emoji"]["id"], emoji["id"]);
    assert!(visible[0]["expires_at"].is_string());

    // Expiry is enforced at read time.
    let past = Utc::now() - Duration::seconds(1);
    let expire = "UPDATE user_emoji_status SET expires_at = $1";
    match server.state.postgres_pool() {
        Some(pool) => sqlx::query(expire)
            .bind(past)
            .execute(pool)
            .await
            .map(|_| ()),
        None => sqlx::query(expire)
            .bind(past)
            .execute(server.state.pool())
            .await
            .map(|_| ()),
    }
    .unwrap();
    let (_, expired) = get(&server, &bob, &path).await;
    assert_eq!(expired, json!([]));

    // Without expiry it stays until cleared.
    let (status, _) = put_status(&server, &alice, json!({ "custom_emoji_id": emoji["id"] })).await;
    assert_eq!(status, StatusCode::OK);
    let (_, visible) = get(&server, &bob, &path).await;
    assert!(visible[0]["expires_at"].is_null());
    let cleared = server
        .client
        .delete(server.url("/api/users/me/emoji-status"))
        .bearer_auth(&alice)
        .send()
        .await
        .unwrap();
    assert_eq!(cleared.status(), StatusCode::NO_CONTENT);
    let (_, gone) = get(&server, &bob, &path).await;
    assert_eq!(gone, json!([]));
}
