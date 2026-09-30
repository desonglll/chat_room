//! Shared calls for the TG-404 scheduled/silent message tests, on top of `poll_support`'s
//! served-state harness.

#![allow(dead_code)]

use chrono::{Duration, Utc};
use reqwest::{Method, StatusCode};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::poll_support::{call, Account};

pub fn in_seconds(seconds: i64) -> String {
    (Utc::now() + Duration::seconds(seconds)).to_rfc3339()
}

pub async fn schedule(
    base: &str,
    account: &Account,
    chat: Uuid,
    body: Value,
) -> (StatusCode, Value) {
    call(
        Method::POST,
        format!("{base}/api/chats/{chat}/scheduled-messages"),
        &account.token,
        Some(body),
    )
    .await
}

/// Schedule `content` `seconds` from now; must succeed. Returns the scheduled id.
pub async fn schedule_ok(
    base: &str,
    account: &Account,
    chat: Uuid,
    content: &str,
    seconds: i64,
    silent: bool,
) -> Uuid {
    let (status, body) = schedule(
        base,
        account,
        chat,
        json!({ "content": content, "scheduled_at": in_seconds(seconds), "silent": silent }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "schedule: {body}");
    body["id"].as_str().unwrap().parse().unwrap()
}

pub async fn list(base: &str, account: &Account, chat: Uuid) -> (StatusCode, Value) {
    call(
        Method::GET,
        format!("{base}/api/chats/{chat}/scheduled-messages"),
        &account.token,
        None,
    )
    .await
}

pub async fn patch(
    base: &str,
    account: &Account,
    chat: Uuid,
    id: Uuid,
    body: Value,
) -> (StatusCode, Value) {
    call(
        Method::PATCH,
        format!("{base}/api/chats/{chat}/scheduled-messages/{id}"),
        &account.token,
        Some(body),
    )
    .await
}

pub async fn delete(base: &str, account: &Account, chat: Uuid, id: Uuid) -> StatusCode {
    call(
        Method::DELETE,
        format!("{base}/api/chats/{chat}/scheduled-messages/{id}"),
        &account.token,
        None,
    )
    .await
    .0
}

pub async fn send_now(base: &str, account: &Account, chat: Uuid, id: Uuid) -> (StatusCode, Value) {
    call(
        Method::POST,
        format!("{base}/api/chats/{chat}/scheduled-messages/{id}/send-now"),
        &account.token,
        None,
    )
    .await
}

pub async fn get_json(base: &str, account: &Account, path: &str) -> Value {
    let (status, body) = call(Method::GET, format!("{base}{path}"), &account.token, None).await;
    assert_eq!(status, StatusCode::OK, "GET {path}: {body}");
    body
}

/// The caller's conversation row for `chat` from `GET /api/conversations`.
pub async fn conversation(base: &str, account: &Account, chat: Uuid) -> Value {
    let rows = get_json(base, account, "/api/conversations").await;
    rows.as_array()
        .unwrap()
        .iter()
        .find(|row| row["room_id"] == chat.to_string())
        .cloned()
        .expect("the member sees the conversation")
}

/// How many history rows carry `id` for `account`.
pub async fn history_count(base: &str, account: &Account, chat: Uuid, id: Uuid) -> usize {
    crate::poll_support::history(base, account, chat)
        .await
        .iter()
        .filter(|message| message["id"] == id.to_string())
        .count()
}
