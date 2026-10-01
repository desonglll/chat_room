//! TG-601: the server embeds and serves the React client's own service worker (not the
//! retiring stub build.rs wrote while there was none) with a root scope and no caching, plus the
//! manifest the page links to.

use std::sync::Arc;

use chat_room::{build_app_with_web, state::AppState};
use tokio::net::TcpListener;

#[tokio::test]
async fn the_real_service_worker_and_manifest_are_served() {
    let state = Arc::new(AppState::new().await.unwrap());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        axum::serve(listener, build_app_with_web(state, true))
            .await
            .unwrap()
    });

    let response = reqwest::get(format!("{base}/sw.js")).await.unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    assert_eq!(response.headers()["service-worker-allowed"], "/");
    assert_eq!(response.headers()["cache-control"], "no-cache");
    let worker = response.text().await.unwrap();
    for marker in ["tg-shell-v1", "notificationclick", "clear-user-data"] {
        assert!(worker.contains(marker), "sw.js lacks {marker}");
    }
    assert!(
        !worker.contains("registration.unregister"),
        "still the retiring stub"
    );

    let manifest: serde_json::Value = reqwest::get(format!("{base}/manifest.webmanifest"))
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(manifest["start_url"], "/");
    let page = reqwest::get(format!("{base}/chat/anything"))
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(
        page.contains("rel=\"manifest\""),
        "the shell links the manifest"
    );
    task.abort();
}
