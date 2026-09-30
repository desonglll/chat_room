//! Shared harness for the TG-202 channel tests, on top of the TG-406 poll harness (served
//! `AppState`, accounts, JSON calls, WebSocket frames, bulk member seeding).

#![allow(dead_code)]

use futures_util::SinkExt;
use reqwest::{multipart, Method, StatusCode};
use serde_json::{json, Value};
use tokio_tungstenite::tungstenite::Message;
use uuid::Uuid;

pub use crate::poll_support::*;

pub async fn create_channel(base: &str, owner: &Account, title: &str, signatures: bool) -> Value {
    let (status, body) = call(
        Method::POST,
        format!("{base}/api/chats"),
        &owner.token,
        Some(json!({
            "title": title,
            "password": "",
            "join_policy": "open",
            "chat_type": "channel",
            "signatures_enabled": signatures,
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "create channel: {body}");
    body
}

pub fn id_of(value: &Value) -> Uuid {
    value["id"].as_str().unwrap().parse().unwrap()
}

pub async fn subscribe(base: &str, account: &Account, channel: Uuid) -> (StatusCode, Value) {
    call(
        Method::POST,
        format!("{base}/api/chats/{channel}/subscription"),
        &account.token,
        None,
    )
    .await
}

pub async fn unsubscribe(base: &str, account: &Account, channel: Uuid) -> StatusCode {
    call(
        Method::DELETE,
        format!("{base}/api/chats/{channel}/subscription"),
        &account.token,
        None,
    )
    .await
    .0
}

pub async fn get_chat(base: &str, account: &Account, chat: Uuid) -> Value {
    let (status, body) = call(
        Method::GET,
        format!("{base}/api/chats/{chat}"),
        &account.token,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "get chat: {body}");
    body
}

pub async fn my_permissions(base: &str, account: &Account, chat: Uuid) -> Vec<String> {
    let (status, body) = call(
        Method::GET,
        format!("{base}/api/chats/{chat}/permissions"),
        &account.token,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "permissions: {body}");
    body["my_permissions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|key| key.as_str().unwrap().to_string())
        .collect()
}

/// Appoint `member` with exactly `rights` (plus the baseline).
pub async fn appoint(base: &str, owner: &Account, chat: Uuid, member: &Account, rights: &[&str]) {
    let (status, body) = call(
        Method::PUT,
        format!("{base}/api/chats/{chat}/members/{}/admin", member.id),
        &owner.token,
        Some(json!({ "permissions": rights, "custom_title": "" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "appoint: {body}");
}

pub async fn send_frame(socket: &mut Socket, frame: Value) {
    socket.send(Message::Text(frame.to_string())).await.unwrap();
}

pub async fn say(socket: &mut Socket, content: &str) {
    send_frame(socket, json!({ "type": "message", "content": content })).await;
}

/// `POST /api/chats/:id/attachments` with one small text file; the status only.
pub async fn upload_status(base: &str, account: &Account, chat: Uuid) -> StatusCode {
    let part = multipart::Part::bytes(b"channel attachment".to_vec())
        .file_name("note.txt")
        .mime_str("text/plain")
        .unwrap();
    reqwest::Client::new()
        .post(format!("{base}/api/chats/{chat}/attachments"))
        .bearer_auth(&account.token)
        .multipart(multipart::Form::new().part("file", part))
        .send()
        .await
        .unwrap()
        .status()
}

/// The contents of `chat`'s history, oldest first.
pub async fn contents(base: &str, account: &Account, chat: Uuid) -> Vec<String> {
    let mut messages = history(base, account, chat).await;
    messages.reverse();
    messages
        .iter()
        .map(|message| message["content"].as_str().unwrap_or_default().to_string())
        .collect()
}
