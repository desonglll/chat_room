//! TG-1210: a chat title is a display name, as in Telegram. Chats are told apart by id, invite
//! link or @username, so creating or renaming into an existing title succeeds.

use std::sync::Arc;

use chat_room::{build_app, state::AppState};
use reqwest::StatusCode;
use serde_json::{json, Value};
use tokio::net::TcpListener;

mod support;
use support::session_token;

struct TestServer {
    base: String,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn start_server() -> TestServer {
    let state = Arc::new(AppState::new().await.unwrap());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        axum::serve(listener, build_app(state)).await.unwrap();
    });
    TestServer { base, task }
}

async fn create(base: &str, token: &str, title: &str) -> String {
    let response = reqwest::Client::new()
        .post(format!("{base}/api/chats"))
        .bearer_auth(token)
        .json(&json!({ "name": title, "password": "" }))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED, "creating {title:?}");
    let chat: Value = response.json().await.unwrap();
    chat["id"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn creating_and_renaming_into_an_existing_title_succeeds() {
    let server = start_server().await;
    let base = &server.base;
    let alice = session_token(base, "dup-title-alice").await;
    let bob = session_token(base, "dup-title-bob").await;

    let first = create(base, &alice, "lobby").await;
    let second = create(base, &bob, "lobby").await;
    assert_ne!(first, second);
    // The same owner may also reuse one of their own titles.
    let third = create(base, &bob, "lounge").await;
    let renamed = reqwest::Client::new()
        .patch(format!("{base}/api/chats/{third}"))
        .bearer_auth(&bob)
        .json(&json!({ "title": "lobby" }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        renamed.status(),
        StatusCode::OK,
        "rename into an existing title"
    );

    // Anonymous: discovery hides chats the caller already belongs to.
    let found: Vec<Value> = reqwest::get(format!("{base}/api/chats/discover?title=lobby"))
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let mut ids: Vec<&str> = found
        .iter()
        .map(|chat| chat["id"].as_str().unwrap())
        .collect();
    ids.sort_unstable();
    let mut expected = vec![first.as_str(), second.as_str(), third.as_str()];
    expected.sort_unstable();
    assert_eq!(ids, expected, "every chat sharing the title is listed");

    // A member's own list filters by title too and keeps both of Bob's.
    let mine: Vec<Value> = reqwest::Client::new()
        .get(format!("{base}/api/chats?title=lobby"))
        .bearer_auth(&bob)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(mine.len(), 2);
}
