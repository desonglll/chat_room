//! TG-402 test helpers: real and synthetic round-video files and the video note endpoints.
//! The live server, accounts and chats come from TG-505's `privacy_support`; history, frame
//! and direct-chat helpers from TG-401's `voice_support` (read-only reuse of both).

#![allow(dead_code)]

use reqwest::{multipart, StatusCode};
use serde_json::Value;
use uuid::Uuid;

use crate::privacy_support::{Account, TestServer};

/// A real Chromium recording (VP9 + Opus WebM, 384×384, 2.434 s by its last block).
pub const REAL_WEBM: &[u8] = include_bytes!("../fixtures/video_note/chromium-video-note.webm");
pub const REAL_WEBM_MS: u64 = 2_434;
/// A real audio-only voice recording: not a video note.
pub const VOICE_WEBM: &[u8] = include_bytes!("../fixtures/voice/chromium-recorder.webm");
/// The smallest JPEG prefix the server accepts as a thumbnail (SOI + APP0 marker).
pub const JPEG: &[u8] = &[
    0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, b'J', b'F', b'I', b'F', 0xFF, 0xD9,
];

fn ebml(id: &[u8], body: &[u8]) -> Vec<u8> {
    let mut element = id.to_vec();
    element.push(0x01);
    element.extend_from_slice(&(body.len() as u64).to_be_bytes()[1..]);
    element.extend_from_slice(body);
    element
}

/// A MediaRecorder-shaped WebM with one `V_VP8` track whose newest block starts at
/// `duration_ms` (two clusters, so any length fits the 16-bit block offsets).
pub fn synthetic_video_webm(duration_ms: u32, salt: u8) -> Vec<u8> {
    let unknown = [0x01, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF];
    let mut file = ebml(&[0x1A, 0x45, 0xDF, 0xA3], &ebml(&[0x42, 0x82], b"webm"));
    file.extend([0x18, 0x53, 0x80, 0x67]);
    file.extend(unknown);
    file.extend(ebml(
        &[0x15, 0x49, 0xA9, 0x66],
        &ebml(&[0x2A, 0xD7, 0xB1], &[0x0F, 0x42, 0x40]),
    ));
    file.extend(ebml(
        &[0x16, 0x54, 0xAE, 0x6B],
        &ebml(&[0xAE], &ebml(&[0x86], b"V_VP8")),
    ));
    for timecode in [0, duration_ms] {
        file.extend([0x1F, 0x43, 0xB6, 0x75]);
        file.extend(unknown);
        file.extend(ebml(&[0xE7], &timecode.to_be_bytes()));
        let mut block = vec![0x81, 0, 0, 0x80];
        block.extend_from_slice(&[salt; 24]);
        file.extend(ebml(&[0xA3], &block));
    }
    file
}

pub struct VideoNoteForm {
    pub bytes: Vec<u8>,
    pub duration_ms: Option<u32>,
    pub thumbnail: Option<Vec<u8>>,
    pub reply_to: Option<String>,
}

impl VideoNoteForm {
    pub fn real() -> Self {
        VideoNoteForm {
            bytes: REAL_WEBM.to_vec(),
            duration_ms: Some(2_500),
            thumbnail: Some(JPEG.to_vec()),
            reply_to: None,
        }
    }

    pub fn synthetic(duration_ms: u32, salt: u8) -> Self {
        VideoNoteForm {
            bytes: synthetic_video_webm(duration_ms, salt),
            duration_ms: Some(duration_ms),
            thumbnail: None,
            reply_to: None,
        }
    }
}

pub async fn send_video_note(
    server: &TestServer,
    room_id: Uuid,
    account: &Account,
    form: VideoNoteForm,
) -> (StatusCode, Value) {
    let mut multipart = multipart::Form::new().part(
        "file",
        multipart::Part::bytes(form.bytes)
            .file_name("video_note.webm")
            .mime_str("video/webm")
            .unwrap(),
    );
    if let Some(duration) = form.duration_ms {
        multipart = multipart.text("duration_ms", duration.to_string());
    }
    if let Some(thumbnail) = form.thumbnail {
        multipart = multipart.part(
            "thumbnail",
            multipart::Part::bytes(thumbnail)
                .file_name("thumbnail.jpg")
                .mime_str("image/jpeg")
                .unwrap(),
        );
    }
    if let Some(reply_to) = form.reply_to {
        multipart = multipart.text("reply_to", reply_to);
    }
    let response = server
        .client
        .post(server.url(&format!("/api/chats/{room_id}/video_note")))
        .bearer_auth(&account.token)
        .multipart(multipart)
        .send()
        .await
        .unwrap();
    let status = response.status();
    let text = response.text().await.unwrap();
    (status, serde_json::from_str(&text).unwrap_or(Value::Null))
}

pub async fn mark_watched(server: &TestServer, message_id: &str, account: &Account) -> StatusCode {
    server
        .client
        .post(server.url(&format!("/api/messages/{message_id}/video_note/listened")))
        .bearer_auth(&account.token)
        .send()
        .await
        .unwrap()
        .status()
}
