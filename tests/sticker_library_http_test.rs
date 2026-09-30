//! TG-302: the sticker library over HTTP — install, archive, search, favorites, removal,
//! uninstall — and the set-creation rules.

mod sticker_support;

use chat_room::config::AppConfig;
use reqwest::StatusCode;
use serde_json::{json, Value};
use sticker_support::{http, webp_lossless};

#[tokio::test]
async fn library_recents_favorites_and_search_over_http() {
    let server = http::start(AppConfig::default()).await;
    let (owner, crab) = server
        .seeded_sticker("sticker-library-owner", "library_pack")
        .await;
    let (status, cat) = server
        .upload(&owner, "library_pack", webp_lossless(512, 512), "🐱 🦀")
        .await;
    assert_eq!(status, StatusCode::CREATED);
    let fan = server.token("sticker-library-fan").await;
    let set_id = crab["set_id"].as_str().unwrap();

    let get = |path: &'static str| {
        let request = server.client.get(server.url(path)).bearer_auth(&fan);
        async move { request.send().await.unwrap().json::<Value>().await.unwrap() }
    };
    assert_eq!(
        get("/api/stickers/search?emoji=🦀").await,
        json!([]),
        "search covers installed sets only"
    );

    let installed: Value = server
        .client
        .put(server.url(&format!("/api/stickers/installed/{set_id}")))
        .bearer_auth(&fan)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(installed["sets"][0]["short_name"], "library_pack");
    assert_eq!(installed["sets"][0]["installed"], true);
    assert_eq!(
        installed["sets"][0]["stickers"].as_array().unwrap().len(),
        2
    );
    assert_eq!(installed["revision"], 1);

    let found = get("/api/stickers/search?emoji=🦀").await;
    let ids: Vec<&Value> = found.as_array().unwrap().iter().map(|s| &s["id"]).collect();
    assert_eq!(
        ids,
        vec![&crab["id"], &cat["id"]],
        "set order, then sticker order"
    );

    let archived: Value = server
        .client
        .patch(server.url(&format!("/api/stickers/installed/{set_id}")))
        .bearer_auth(&fan)
        .json(&json!({ "archived": true }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(archived["sets"][0]["archived"], true);
    assert_eq!(
        get("/api/stickers/search?emoji=🦀").await,
        json!([]),
        "archived sets are not searched"
    );

    for sticker in [&crab, &cat] {
        let status = server
            .client
            .put(server.url(&format!(
                "/api/stickers/favorites/{}",
                sticker["id"].as_str().unwrap()
            )))
            .bearer_auth(&fan)
            .send()
            .await
            .unwrap()
            .status();
        assert_eq!(status, StatusCode::NO_CONTENT);
    }
    let favorites = get("/api/stickers/favorites").await;
    assert_eq!(favorites[0]["id"], cat["id"], "newest favorite first");
    assert_eq!(favorites.as_array().unwrap().len(), 2);

    // Removing a sticker from its set hides it everywhere it was listed.
    let status = server
        .client
        .delete(server.url(&format!(
            "/api/sticker-sets/library_pack/stickers/{}",
            cat["id"].as_str().unwrap()
        )))
        .bearer_auth(&owner)
        .send()
        .await
        .unwrap()
        .status();
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(
        get("/api/stickers/favorites")
            .await
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        server.fetch(cat["file_url"].as_str().unwrap()).await.0,
        StatusCode::NOT_FOUND
    );

    let uninstalled: Value = server
        .client
        .delete(server.url(&format!("/api/stickers/installed/{set_id}")))
        .bearer_auth(&fan)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(uninstalled["sets"], json!([]));

    let duplicate = server
        .client
        .post(server.url("/api/sticker-sets"))
        .bearer_auth(&fan)
        .json(&json!({ "short_name": "LIBRARY_pack", "title": "Copy" }))
        .send()
        .await
        .unwrap();
    assert_eq!(duplicate.status(), StatusCode::CONFLICT);
    let invalid = server
        .client
        .post(server.url("/api/sticker-sets"))
        .bearer_auth(&fan)
        .json(&json!({ "short_name": "9lives", "title": "Bad" }))
        .send()
        .await
        .unwrap();
    assert_eq!(invalid.status(), StatusCode::BAD_REQUEST);
    let anonymous = server
        .client
        .get(server.url("/api/stickers/installed"))
        .send()
        .await
        .unwrap();
    assert_eq!(anonymous.status(), StatusCode::UNAUTHORIZED);
}
