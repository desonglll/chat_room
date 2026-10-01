//! TG-507 wallpapers over HTTP on SQLite and PostgreSQL: global and per-chat scopes (only chats
//! the caller can read), validation per kind, image upload served to its owner only, and reset.

use std::sync::Arc;

use chat_room::state::AppState;
use reqwest::{multipart, Method, StatusCode};
use serde_json::json;

mod chat_admin_support;

use chat_admin_support::{serve, with_postgres};

const PNG: &[u8] =
    b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR\0\0\0\x01\0\0\0\x01\x08\x06\0\0\0\x1f\x15\xc4\x89";

async fn wallpapers_scenario(state: Arc<AppState>) {
    let server = serve(state).await;
    let alice = server.register("wp-alice").await;
    let bob = server.register("wp-bob").await;
    let mine = server.create_group(&alice, "wp-mine").await;
    let foreign = server.create_group(&bob, "wp-foreign").await;
    let path = |scope: &str| format!("/api/users/me/wallpapers/{scope}");

    let (status, list) = server.get("/api/users/me/wallpapers", &alice.token).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(list, json!([]));

    let (status, global) = server
        .put(
            &path("global"),
            &alice.token,
            json!({ "kind": "preset", "preset": "sunset" }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{global}");
    assert_eq!(global["preset"], "sunset");
    for bad in [
        json!({ "kind": "preset", "preset": "nope" }),
        json!({ "kind": "color", "colors": ["red"] }),
        json!({ "kind": "gradient", "colors": ["#000000"] }),
        json!({ "kind": "color", "colors": ["#000000"], "dim": 90 }),
        json!({ "kind": "image" }),
    ] {
        assert_eq!(
            server.put(&path("global"), &alice.token, bad).await.0,
            StatusCode::BAD_REQUEST
        );
    }

    let gradient =
        json!({ "kind": "gradient", "colors": ["#112233", "#445566", "#778899"], "dim": 20 });
    let (status, chat) = server
        .put(&path(&mine), &alice.token, gradient.clone())
        .await;
    assert_eq!(status, StatusCode::OK, "{chat}");
    assert_eq!(chat["colors"].as_array().unwrap().len(), 3);
    assert_eq!(
        server
            .put(&path(&foreign), &alice.token, gradient.clone())
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        server
            .put(&path("not-a-chat"), &alice.token, gradient)
            .await
            .0,
        StatusCode::NOT_FOUND
    );

    // An uploaded image is served to its owner only.
    let form = multipart::Form::new()
        .part(
            "file",
            multipart::Part::bytes(PNG.to_vec()).file_name("w.png"),
        )
        .text("blur", "true")
        .text("dim", "30");
    let response = reqwest::Client::new()
        .post(format!("{}{}/image", server.base, path(&mine)))
        .bearer_auth(&alice.token)
        .multipart(form)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let image: serde_json::Value = response.json().await.unwrap();
    assert_eq!(image["kind"], "image");
    assert_eq!(image["blur"], true);
    assert_eq!(image["dim"], 30);
    let url = image["image_url"].as_str().unwrap().to_string();
    let fetch = |token: String| {
        let url = format!("{}{url}", server.base);
        async move {
            reqwest::Client::new()
                .get(url)
                .bearer_auth(token)
                .send()
                .await
                .unwrap()
        }
    };
    let own = fetch(alice.token.clone()).await;
    assert_eq!(own.status(), StatusCode::OK);
    assert_eq!(own.bytes().await.unwrap().as_ref(), PNG);
    assert_eq!(
        fetch(bob.token.clone()).await.status(),
        StatusCode::NOT_FOUND
    );
    let not_image = multipart::Form::new().part("file", multipart::Part::bytes(b"hello".to_vec()));
    let refused = reqwest::Client::new()
        .post(format!("{}{}/image", server.base, path(&mine)))
        .bearer_auth(&alice.token)
        .multipart(not_image)
        .send()
        .await
        .unwrap();
    assert_eq!(refused.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);

    // Reset.
    let (status, _) = server
        .call(Method::DELETE, &path(&mine), &alice.token, None)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(
        server
            .call(Method::DELETE, &path(&mine), &alice.token, None)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    let (_, list) = server.get("/api/users/me/wallpapers", &alice.token).await;
    assert_eq!(list.as_array().unwrap().len(), 1);
    assert_eq!(list[0]["scope"], "global");
    let (_, others) = server.get("/api/users/me/wallpapers", &bob.token).await;
    assert_eq!(others, json!([]));
}

#[tokio::test]
async fn sqlite_wallpapers_scopes_validation_images_and_reset() {
    wallpapers_scenario(Arc::new(AppState::new().await.unwrap())).await;
}

#[tokio::test]
async fn postgres_wallpapers_scopes_validation_images_and_reset() {
    with_postgres("postgres_chat_wallpapers", wallpapers_scenario).await;
}
