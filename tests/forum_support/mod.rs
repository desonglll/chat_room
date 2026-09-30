//! HTTP / WebSocket helpers for the TG-204 forum-topic tests, on top of the TG-201 ones.

#![allow(dead_code)]

use futures_util::SinkExt;
use reqwest::{multipart, Method, StatusCode};
use serde_json::{json, Value};
use tokio_tungstenite::tungstenite::Message;

use super::chat_admin_support::{next_frame, Account, Server, Socket};

pub mod creator;

pub const BLUE: i64 = 0x6FB9F0;
pub const RED: i64 = 0xFB6F5F;

/// Turn the forum on; answers the chat descriptor.
pub async fn enable_forum(server: &Server, chat: &str, owner: &Account) -> Value {
    let (status, chat) = server
        .put(
            &format!("/api/chats/{chat}/forum"),
            &owner.token,
            json!({ "enabled": true }),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{chat}");
    chat
}

pub async fn topics(server: &Server, chat: &str, account: &Account) -> Value {
    let (status, list) = server
        .get(&format!("/api/chats/{chat}/topics"), &account.token)
        .await;
    assert_eq!(status, StatusCode::OK, "{list}");
    list
}

/// One topic of the viewer's list, by title.
pub async fn topic(server: &Server, chat: &str, account: &Account, title: &str) -> Value {
    let list = topics(server, chat, account).await;
    list["topics"]
        .as_array()
        .unwrap()
        .iter()
        .find(|topic| topic["title"] == title)
        .unwrap_or_else(|| panic!("topic {title} in {list}"))
        .clone()
}

pub async fn general(server: &Server, chat: &str, account: &Account) -> Value {
    let list = topics(server, chat, account).await;
    list["topics"]
        .as_array()
        .unwrap()
        .iter()
        .find(|topic| topic["is_general"] == true)
        .unwrap()
        .clone()
}

pub async fn create_topic(
    server: &Server,
    chat: &str,
    account: &Account,
    body: Value,
) -> (StatusCode, Value) {
    server
        .call(
            Method::POST,
            &format!("/api/chats/{chat}/topics"),
            &account.token,
            Some(body),
        )
        .await
}

/// Create a topic that must succeed; answers its id.
pub async fn new_topic(server: &Server, chat: &str, account: &Account, title: &str) -> String {
    let (status, topic) = create_topic(server, chat, account, json!({ "title": title })).await;
    assert_eq!(status, StatusCode::CREATED, "{topic}");
    topic["id"].as_str().unwrap().to_string()
}

pub async fn patch_topic(
    server: &Server,
    chat: &str,
    topic: &str,
    account: &Account,
    body: Value,
) -> (StatusCode, Value) {
    server
        .call(
            Method::PATCH,
            &format!("/api/chats/{chat}/topics/{topic}"),
            &account.token,
            Some(body),
        )
        .await
}

pub async fn send_in_topic(socket: &mut Socket, content: &str, topic: Option<&str>) {
    let mut frame = json!({ "type": "message", "content": content });
    if let Some(topic) = topic {
        frame["topic_id"] = json!(topic);
    }
    socket.send(Message::Text(frame.to_string())).await.unwrap();
}

/// The next `broadcast` frame carrying `content`, skipping everything else.
pub async fn broadcast_of(socket: &mut Socket, content: &str) -> Value {
    loop {
        let frame = next_frame(socket, "broadcast").await;
        if frame["content"] == content {
            return frame;
        }
    }
}

pub async fn topic_history(server: &Server, chat: &str, topic: &str, token: &str) -> Vec<Value> {
    let (status, page) = server
        .get(&format!("/api/chats/{chat}/topics/{topic}/messages"), token)
        .await;
    assert_eq!(status, StatusCode::OK, "{page}");
    page.as_array().unwrap().clone()
}

pub fn contents(page: &[Value]) -> Vec<&str> {
    page.iter()
        .map(|message| message["content"].as_str().unwrap())
        .collect()
}

pub async fn chat_history(server: &Server, chat: &str, token: &str) -> Vec<Value> {
    let (status, page) = server
        .get(&format!("/api/chats/{chat}/messages"), token)
        .await;
    assert_eq!(status, StatusCode::OK);
    page.as_array().unwrap().clone()
}

/// A single-shot multipart upload; answers the status and body.
pub async fn upload(
    server: &Server,
    chat: &str,
    token: &str,
    topic: Option<&str>,
) -> (StatusCode, Value) {
    let part = multipart::Part::bytes(b"forum attachment bytes".to_vec())
        .file_name("notes.txt")
        .mime_str("text/plain")
        .unwrap();
    let mut form = multipart::Form::new()
        .part("file", part)
        .text("content", "a file");
    if let Some(topic) = topic {
        form = form.text("topic_id", topic.to_string());
    }
    let response = server
        .client
        .post(format!("{}/api/chats/{chat}/attachments", server.base))
        .bearer_auth(token)
        .multipart(form)
        .send()
        .await
        .unwrap();
    let status = response.status();
    (status, response.json().await.unwrap_or(Value::Null))
}

/// The resumable path: create, one chunk, complete with `topic_id`.
pub async fn chunked_upload(
    server: &Server,
    chat: &str,
    token: &str,
    topic: Option<&str>,
) -> StatusCode {
    let bytes = b"chunked forum bytes".to_vec();
    let (status, created) = server
        .call(
            Method::POST,
            &format!("/api/chats/{chat}/attachments/uploads"),
            token,
            Some(json!({ "file_name": "c.txt", "mime_type": "text/plain",
                "size_bytes": bytes.len(), "fingerprint": uuid::Uuid::new_v4().to_string() })),
        )
        .await;
    assert!(status.is_success(), "{status} {created}");
    let upload_id = created["upload_id"].as_str().unwrap();
    let response = server
        .client
        .put(format!(
            "{}/api/attachments/uploads/{upload_id}/chunks?offset=0",
            server.base
        ))
        .bearer_auth(token)
        .body(bytes)
        .send()
        .await
        .unwrap();
    assert!(response.status().is_success(), "{}", response.status());
    let (status, _) = server
        .call(
            Method::POST,
            &format!("/api/attachments/uploads/{upload_id}/complete"),
            token,
            Some(json!({ "content": "chunked", "topic_id": topic })),
        )
        .await;
    status
}

pub async fn poll(server: &Server, chat: &str, token: &str, topic: Option<&str>) -> StatusCode {
    server
        .call(
            Method::POST,
            &format!("/api/chats/{chat}/polls"),
            token,
            Some(json!({ "question": "Lunch?", "options": ["yes", "no"], "topic_id": topic })),
        )
        .await
        .0
}

/// The chat-level unread count from the chat list.
pub async fn chat_unread(server: &Server, chat: &str, token: &str) -> i64 {
    let (_, list) = server.get("/api/chats", token).await;
    list.as_array()
        .unwrap()
        .iter()
        .find(|item| item["id"] == chat)
        .unwrap()["unread_count"]
        .as_i64()
        .unwrap()
}
