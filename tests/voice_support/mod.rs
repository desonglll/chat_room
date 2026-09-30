//! TG-401 test helpers: synthetic recorder files, the voice endpoints, and socket reads.
//! The live server, accounts and chats come from TG-505's `privacy_support` (read-only reuse).

#![allow(dead_code)]

use std::time::Duration;

use futures_util::StreamExt;
use reqwest::{multipart, StatusCode};
use serde_json::{json, Value};
use tokio_tungstenite::tungstenite::Message;
use uuid::Uuid;

use crate::privacy_support::{Account, Socket, TestServer};

fn ebml(id: &[u8], body: &[u8]) -> Vec<u8> {
    let mut element = id.to_vec();
    element.push(0x01);
    element.extend_from_slice(&(body.len() as u64).to_be_bytes()[1..]);
    element.extend_from_slice(body);
    element
}

/// A MediaRecorder-shaped WebM/Opus file (live Segment and Cluster sizes, no Duration) whose
/// newest block starts at `duration_ms`. Distinct `salt`s give distinct content hashes.
pub fn recorder_webm(duration_ms: u16, salt: u8) -> Vec<u8> {
    let unknown = [0x01, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF];
    let mut file = ebml(&[0x1A, 0x45, 0xDF, 0xA3], &ebml(&[0x42, 0x82], b"webm"));
    file.extend([0x18, 0x53, 0x80, 0x67]);
    file.extend(unknown);
    file.extend(ebml(
        &[0x15, 0x49, 0xA9, 0x66],
        &ebml(&[0x2A, 0xD7, 0xB1], &[0x0F, 0x42, 0x40]),
    ));
    file.extend([0x1F, 0x43, 0xB6, 0x75]);
    file.extend(unknown);
    file.extend(ebml(&[0xE7], &[0]));
    for relative in [0u16, duration_ms / 2, duration_ms] {
        let mut block = vec![0x81];
        block.extend_from_slice(&relative.to_be_bytes());
        block.push(0x80);
        block.extend_from_slice(&[salt; 24]);
        file.extend(ebml(&[0xA3], &block));
    }
    file
}

/// A ramp 0..=31..0 — asymmetric enough that an off-by-one in packing would show.
pub fn waveform() -> Vec<u8> {
    (0..100u16)
        .map(|i| if i < 50 { i * 31 / 49 } else { (99 - i) * 31 / 49 } as u8)
        .collect()
}

pub fn waveform_field(samples: &[u8]) -> String {
    samples
        .iter()
        .map(u8::to_string)
        .collect::<Vec<_>>()
        .join(",")
}

pub struct VoiceForm {
    pub bytes: Vec<u8>,
    pub mime: &'static str,
    pub waveform: String,
    pub duration_ms: Option<u32>,
}

impl VoiceForm {
    pub fn webm(duration_ms: u16, salt: u8) -> Self {
        VoiceForm {
            bytes: recorder_webm(duration_ms, salt),
            mime: "audio/webm",
            waveform: waveform_field(&waveform()),
            duration_ms: Some(u32::from(duration_ms)),
        }
    }
}

pub async fn send_voice(
    server: &TestServer,
    room_id: Uuid,
    account: &Account,
    form: VoiceForm,
) -> (StatusCode, Value) {
    let mut multipart = multipart::Form::new()
        .part(
            "file",
            multipart::Part::bytes(form.bytes)
                .file_name("voice.webm")
                .mime_str(form.mime)
                .unwrap(),
        )
        .text("waveform", form.waveform);
    if let Some(duration) = form.duration_ms {
        multipart = multipart.text("duration_ms", duration.to_string());
    }
    let response = server
        .client
        .post(server.url(&format!("/api/chats/{room_id}/voice")))
        .bearer_auth(&account.token)
        .multipart(multipart)
        .send()
        .await
        .unwrap();
    let status = response.status();
    let text = response.text().await.unwrap();
    (status, serde_json::from_str(&text).unwrap_or(Value::Null))
}

pub async fn mark_listened(server: &TestServer, message_id: &str, account: &Account) -> StatusCode {
    server
        .client
        .post(server.url(&format!("/api/messages/{message_id}/voice/listened")))
        .bearer_auth(&account.token)
        .send()
        .await
        .unwrap()
        .status()
}

/// The chat's history as `account` sees it, newest last.
pub async fn history(server: &TestServer, room_id: Uuid, account: &Account) -> Vec<Value> {
    let response = server
        .client
        .get(server.url(&format!("/api/chats/{room_id}/messages")))
        .bearer_auth(&account.token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    response.json().await.unwrap()
}

pub fn find<'a>(messages: &'a [Value], id: &str) -> &'a Value {
    messages
        .iter()
        .find(|message| message["id"] == id)
        .unwrap_or_else(|| panic!("message {id} not in history"))
}

pub async fn direct_chat(server: &TestServer, owner: &Account, peer: &Account) -> Uuid {
    let body: Value = server
        .client
        .post(server.url("/api/direct-chats"))
        .bearer_auth(&owner.token)
        .json(&json!({ "user_id": peer.id }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    body["room_id"].as_str().unwrap().parse().unwrap()
}

/// Read frames until one of `kind` arrives (other kinds are skipped).
pub async fn next_of(socket: &mut Socket, kind: &str) -> Value {
    loop {
        let frame = tokio::time::timeout(Duration::from_secs(5), socket.next())
            .await
            .unwrap_or_else(|_| panic!("timed out waiting for a {kind} frame"))
            .expect("WebSocket ended")
            .expect("WebSocket error");
        let Message::Text(text) = frame else { continue };
        let value: Value = serde_json::from_str(&text).unwrap();
        if value["type"] == kind {
            return value;
        }
    }
}

/// Every frame `socket` receives before a System marker broadcast now on `room_id`. The chat
/// channel is one ordered broadcast, so a frame sent before this call (e.g. inside a request
/// that has already answered) that is missing here was never delivered to this socket.
pub async fn frames_before_marker(
    server: &TestServer,
    room_id: Uuid,
    socket: &mut Socket,
    marker: &str,
) -> Vec<Value> {
    server
        .state
        .broadcast(
            room_id,
            chat_room::models::ChatMessage::System {
                content: marker.into(),
                members: None,
                participants: None,
            },
        )
        .await;
    let mut seen = Vec::new();
    loop {
        let frame = tokio::time::timeout(Duration::from_secs(5), socket.next())
            .await
            .expect("timed out waiting for the marker")
            .expect("WebSocket ended")
            .expect("WebSocket error");
        let Message::Text(text) = frame else { continue };
        let value: Value = serde_json::from_str(&text).unwrap();
        if value["type"] == "system" && value["content"] == marker {
            return seen;
        }
        seen.push(value);
    }
}
