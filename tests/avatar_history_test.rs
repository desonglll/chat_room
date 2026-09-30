//! TG-511 profile photo history. Every upload is kept, newest first; an older photo can be set
//! as the main one; deleting the main photo promotes the next newest (history order stays
//! correct); switching to an emoji keeps the photos; the privacy rule guards the history.

use std::sync::Arc;

use chat_room::state::AppState;
use reqwest::{multipart, Method, StatusCode};
use serde_json::{json, Value};

mod chat_admin_support;

use chat_admin_support::{serve, with_postgres, Account, Server};

const PNG: &[u8] = &[
    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 6, 0,
    0, 0, 31, 21, 196, 137, 0, 0, 0, 13, 73, 68, 65, 84, 8, 215, 99, 248, 207, 192, 240, 31, 0, 5,
    0, 1, 255, 137, 153, 61, 29, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
];

async fn upload(server: &Server, account: &Account) {
    let form = multipart::Form::new().part(
        "file",
        multipart::Part::bytes(PNG.to_vec())
            .file_name("avatar.png")
            .mime_str("image/png")
            .unwrap(),
    );
    let response = server
        .client
        .post(format!("{}/api/users/me/avatar", server.base))
        .bearer_auth(&account.token)
        .multipart(form)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    // Distinct `created_at` values keep "newest first" unambiguous.
    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
}

async fn history(server: &Server, owner: &Account, viewer: &Account) -> (StatusCode, Vec<Value>) {
    let (status, body) = server
        .get(&format!("/api/users/{}/avatars", owner.id), &viewer.token)
        .await;
    (status, body.as_array().cloned().unwrap_or_default())
}

async fn avatar_history_scenario(state: Arc<AppState>) {
    let server = serve(state).await;
    let alice = server.register("av-alice").await;
    let bob = server.register("av-bob").await;

    for _ in 0..3 {
        upload(&server, &alice).await;
    }
    let (status, photos) = history(&server, &alice, &bob).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(photos.len(), 3, "every upload is kept");
    let ids: Vec<String> = photos
        .iter()
        .map(|p| p["id"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(
        photos[0]["is_current"], true,
        "the newest is the main photo"
    );
    let (status, _) = server
        .get(
            &format!("/api/users/{}/avatars/{}", alice.id, ids[2]),
            &bob.token,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "an older photo is downloadable");

    // Set the oldest as main.
    let (status, user) = server
        .call(
            Method::PUT,
            &format!("/api/users/me/avatars/{}/main", ids[2]),
            &alice.token,
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{user}");
    let (_, photos) = history(&server, &alice, &bob).await;
    assert_eq!(
        photos.iter().find(|p| p["is_current"] == true).unwrap()["id"],
        ids[2].as_str()
    );

    // Deleting the main photo promotes the next newest; order stays newest first.
    let (status, _) = server
        .call(
            Method::DELETE,
            &format!("/api/users/me/avatars/{}", ids[2]),
            &alice.token,
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let (_, photos) = history(&server, &alice, &bob).await;
    let remaining: Vec<&str> = photos.iter().map(|p| p["id"].as_str().unwrap()).collect();
    assert_eq!(remaining, vec![ids[0].as_str(), ids[1].as_str()]);
    assert_eq!(photos[0]["is_current"], true);

    // Bob cannot delete Alice's photo.
    let (status, _) = server
        .call(
            Method::DELETE,
            &format!("/api/users/me/avatars/{}", ids[0]),
            &bob.token,
            None,
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // Switching to an emoji keeps the photos in the history.
    let (status, _) = server
        .call(
            Method::PATCH,
            "/api/users/me",
            &alice.token,
            Some(json!({ "avatar_emoji": "🦊" })),
        )
        .await;
    assert!(status.is_success(), "{status}");
    let (_, photos) = history(&server, &alice, &bob).await;
    assert_eq!(photos.len(), 2);
    assert!(photos.iter().all(|p| p["is_current"] == false));

    // The privacy rule guards the history too.
    let (status, _) = server
        .call(
            Method::PUT,
            "/api/users/me/privacy/profile_photo",
            &alice.token,
            Some(json!({ "tier": "nobody" })),
        )
        .await;
    assert!(status.is_success(), "privacy: {status}");
    let (status, _) = history(&server, &alice, &bob).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, own) = history(&server, &alice, &alice).await;
    assert_eq!(
        (status, own.len()),
        (StatusCode::OK, 2),
        "the owner always sees their own"
    );
}

#[tokio::test]
async fn sqlite_avatar_history_keeps_orders_promotes_and_respects_privacy() {
    avatar_history_scenario(Arc::new(AppState::new().await.unwrap())).await;
}

#[tokio::test]
async fn postgres_avatar_history_keeps_orders_promotes_and_respects_privacy() {
    with_postgres("postgres_avatar_history", avatar_history_scenario).await;
}
