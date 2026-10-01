//! TG-407 locations over HTTP on SQLite and PostgreSQL: validation, static locations in history,
//! live locations that only their sender can move while live, stopping (the last point stays,
//! no further points are accepted), the merged live view, and read authorization.

mod chat_admin_support;
mod poll_support;

use std::sync::Arc;

use chat_admin_support::with_postgres;
use chat_room::state::AppState;
use poll_support::{call, create_chat, history, join, register, serve};
use reqwest::{Method, StatusCode};
use serde_json::json;

async fn locations_scenario(state: Arc<AppState>) {
    let server = serve(state).await;
    let base = &server.base;
    let owner = register(base, "loc-owner").await;
    let bob = register(base, "loc-bob").await;
    let outsider = register(base, "loc-outsider").await;
    let chat = create_chat(base, &owner, "loc chat").await;
    join(base, &bob, chat).await;
    let send = |token: String, body: serde_json::Value| {
        let url = format!("{base}/api/chats/{chat}/location-messages");
        async move { call(Method::POST, url, &token, Some(body)).await }
    };

    for bad in [
        json!({ "latitude": 91.0, "longitude": 0.0 }),
        json!({ "latitude": 0.0, "longitude": 181.0 }),
        json!({ "latitude": 0.0, "longitude": 0.0, "accuracy_m": -1.0 }),
        json!({ "latitude": 0.0, "longitude": 0.0, "live_seconds": 10 }),
    ] {
        assert_eq!(
            send(owner.token.clone(), bad).await.0,
            StatusCode::BAD_REQUEST
        );
    }
    assert_eq!(
        send(
            outsider.token.clone(),
            json!({ "latitude": 1.0, "longitude": 2.0 })
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );

    // Static.
    let (status, pin) = send(
        owner.token.clone(),
        json!({ "latitude": 31.2304, "longitude": 121.4737, "accuracy_m": 12.5, "title": "外滩" }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{pin}");
    assert_eq!(pin["media_kind"], "location");
    assert_eq!(pin["location"]["latitude"], 31.2304);
    assert!(pin["location"].get("live_until").is_none());

    // Live: only its sender moves it, only while live.
    let (status, live) = send(
        owner.token.clone(),
        json!({ "latitude": 1.0, "longitude": 2.0, "live_seconds": 900 }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{live}");
    let live_id = live["id"].as_str().unwrap().to_string();
    let live_url = format!("{base}/api/chats/{chat}/live-locations/{live_id}");
    let point = json!({ "latitude": 1.5, "longitude": 2.5, "heading": 90 });
    assert_eq!(
        call(
            Method::PUT,
            live_url.clone(),
            &bob.token,
            Some(point.clone())
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    let (status, moved) = call(Method::PUT, live_url.clone(), &owner.token, Some(point)).await;
    assert_eq!(status, StatusCode::OK, "{moved}");
    assert_eq!(moved["latitude"], 1.5);
    assert_eq!(moved["heading"], 90);

    let list_url = format!("{base}/api/chats/{chat}/live-locations");
    let (status, active) = call(Method::GET, list_url.clone(), &bob.token, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(active.as_array().unwrap().len(), 1);
    assert_eq!(active[0]["message_id"], live["id"]);
    assert_eq!(
        call(Method::GET, list_url.clone(), &outsider.token, None)
            .await
            .0,
        StatusCode::NOT_FOUND
    );

    // Stop: the last point stays, nothing more is accepted, nothing is listed.
    let (status, stopped) = call(Method::DELETE, live_url.clone(), &owner.token, None).await;
    assert_eq!(status, StatusCode::OK, "{stopped}");
    assert_eq!(stopped["latitude"], 1.5);
    assert_eq!(
        call(
            Method::PUT,
            live_url.clone(),
            &owner.token,
            Some(json!({ "latitude": 9.0, "longitude": 9.0 }))
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        call(Method::DELETE, live_url, &owner.token, None).await.0,
        StatusCode::NOT_FOUND
    );
    let (_, active) = call(Method::GET, list_url, &bob.token, None).await;
    assert!(active.as_array().unwrap().is_empty());
    let messages = history(base, &bob, chat).await;
    let shown = messages
        .iter()
        .find(|message| message["id"] == live["id"])
        .unwrap();
    assert_eq!(shown["location"]["latitude"], 1.5);
    assert!(shown["location"]["live_until"].is_string());
}

#[tokio::test]
async fn sqlite_location_messages_static_live_and_stop() {
    locations_scenario(Arc::new(AppState::new().await.unwrap())).await;
}

#[tokio::test]
async fn postgres_location_messages_static_live_and_stop() {
    with_postgres("postgres_location_messages", locations_scenario).await;
}
