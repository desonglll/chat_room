//! TG-508 notification settings over HTTP: defaults per chat type, per-chat exceptions (only for
//! chats the viewer can read; all-null removes one), validation. The precedence rule itself is
//! the unit-tested matrix in `src/notifications/exceptions.rs`.

use std::sync::Arc;

use chat_room::state::AppState;
use reqwest::{Method, StatusCode};
use serde_json::json;

mod chat_admin_support;

use chat_admin_support::{serve, with_postgres};

async fn notification_settings_scenario(state: Arc<AppState>) {
    let server = serve(state).await;
    let alice = server.register("ns-alice").await;
    let bob = server.register("ns-bob").await;
    let chat = server.create_group(&alice, "ns-group").await;

    // Defaults: all three scopes, on/preview/default sound until changed.
    let (status, settings) = server
        .get("/api/users/me/notification-settings", &alice.token)
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(settings["defaults"].as_array().unwrap().len(), 3);
    assert!(settings["defaults"]
        .as_array()
        .unwrap()
        .iter()
        .all(|d| d["enabled"] == true));

    let (status, _) = server
        .call(
            Method::PUT,
            "/api/users/me/notification-settings/defaults/group",
            &alice.token,
            Some(json!({ "enabled": false, "preview": false, "sound": "chime" })),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = server
        .call(
            Method::PUT,
            "/api/users/me/notification-settings/defaults/group",
            &alice.token,
            Some(json!({ "enabled": true, "preview": true, "sound": "air-horn" })),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "unknown sound");
    let (status, _) = server
        .call(
            Method::PUT,
            "/api/users/me/notification-settings/defaults/robots",
            &alice.token,
            Some(json!({ "enabled": true, "preview": true, "sound": "default" })),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "unknown scope");

    // An exception for one chat overrides the (now disabled) group default.
    let path = format!("/api/chats/{chat}/notification-exception");
    let (status, _) = server
        .call(
            Method::PUT,
            &path,
            &alice.token,
            Some(json!({ "enabled": true, "sound": "pop" })),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let (_, settings) = server
        .get("/api/users/me/notification-settings", &alice.token)
        .await;
    let group = settings["defaults"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["scope"] == "group")
        .unwrap()
        .clone();
    assert_eq!(
        (group["enabled"].clone(), group["sound"].clone()),
        (json!(false), json!("chime"))
    );
    let exceptions = settings["exceptions"].as_array().unwrap();
    assert_eq!(exceptions.len(), 1);
    assert_eq!(exceptions[0]["chat_id"], chat.as_str());
    assert_eq!(exceptions[0]["chat_title"], "ns-group");
    assert_eq!(exceptions[0]["enabled"], true);
    assert_eq!(exceptions[0]["sound"], "pop");

    // A non-member cannot set one; all-null removes it.
    let (status, _) = server
        .call(
            Method::PUT,
            &path,
            &bob.token,
            Some(json!({ "enabled": false })),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = server
        .call(Method::PUT, &path, &alice.token, Some(json!({})))
        .await;
    assert_eq!(status, StatusCode::OK);
    let (_, settings) = server
        .get("/api/users/me/notification-settings", &alice.token)
        .await;
    assert!(settings["exceptions"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn sqlite_notification_defaults_and_exceptions() {
    notification_settings_scenario(Arc::new(AppState::new().await.unwrap())).await;
}

#[tokio::test]
async fn postgres_notification_defaults_and_exceptions() {
    with_postgres(
        "postgres_notification_exceptions",
        notification_settings_scenario,
    )
    .await;
}
