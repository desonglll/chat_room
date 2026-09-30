//! TG-305 fixtures: byte-exact animation headers built in code (the server reads headers
//! only), and the GIF HTTP calls the tests repeat on top of the sticker scratch server.
#![allow(dead_code)]

use reqwest::StatusCode;
use serde_json::{json, Value};

use super::sticker_support::http::Server;

/// A GIF89a with the given logical screen and a trailer; enough for header validation.
pub fn gif(width: u16, height: u16) -> Vec<u8> {
    let mut bytes = b"GIF89a".to_vec();
    bytes.extend_from_slice(&width.to_le_bytes());
    bytes.extend_from_slice(&height.to_le_bytes());
    bytes.extend_from_slice(&[0, 0, 0, 0x3b]);
    bytes
}

fn mp4_box(kind: &[u8; 4], payload: &[u8]) -> Vec<u8> {
    let mut bytes = ((payload.len() + 8) as u32).to_be_bytes().to_vec();
    bytes.extend_from_slice(kind);
    bytes.extend_from_slice(payload);
    bytes
}

fn track(handler: &[u8; 4], codec: &[u8; 4], width: u32, height: u32) -> Vec<u8> {
    // tkhd v0: 84 bytes of fields, the last 8 are width/height in 16.16 fixed point.
    let mut tkhd = vec![0; 76];
    tkhd.extend_from_slice(&(width << 16).to_be_bytes());
    tkhd.extend_from_slice(&(height << 16).to_be_bytes());
    let mut hdlr = vec![0; 8];
    hdlr.extend_from_slice(handler);
    hdlr.extend_from_slice(&[0; 13]);
    let mut stsd = vec![0, 0, 0, 0, 0, 0, 0, 1];
    stsd.extend(mp4_box(codec, &[0; 8]));
    let stbl = mp4_box(b"stbl", &mp4_box(b"stsd", &stsd));
    let minf = mp4_box(b"minf", &stbl);
    let mut mdia = mp4_box(b"hdlr", &hdlr);
    mdia.extend(minf);
    let mut trak = mp4_box(b"tkhd", &tkhd);
    trak.extend(mp4_box(b"mdia", &mdia));
    mp4_box(b"trak", &trak)
}

pub struct Mp4Spec {
    pub codec: &'static [u8; 4],
    pub width: u32,
    pub height: u32,
    pub duration_ms: u32,
    pub audio: bool,
}

impl Default for Mp4Spec {
    fn default() -> Self {
        Mp4Spec {
            codec: b"avc1",
            width: 480,
            height: 270,
            duration_ms: 2_000,
            audio: false,
        }
    }
}

pub fn mp4(spec: Mp4Spec) -> Vec<u8> {
    let mut ftyp = b"isom".to_vec();
    ftyp.extend_from_slice(&[0, 0, 2, 0]);
    ftyp.extend_from_slice(b"isomavc1");
    // mvhd v0: version/flags, creation, modification, timescale, duration, ...
    let mut mvhd = vec![0; 12];
    mvhd.extend_from_slice(&1000_u32.to_be_bytes());
    mvhd.extend_from_slice(&spec.duration_ms.to_be_bytes());
    mvhd.extend_from_slice(&[0; 80]);
    let mut moov = mp4_box(b"mvhd", &mvhd);
    moov.extend(track(b"vide", spec.codec, spec.width, spec.height));
    if spec.audio {
        moov.extend(track(b"soun", b"mp4a", 0, 0));
    }
    let mut file = mp4_box(b"ftyp", &ftyp);
    file.extend(mp4_box(b"moov", &moov));
    file.extend(mp4_box(b"mdat", &[0; 32]));
    file
}

pub fn valid_mp4() -> Vec<u8> {
    mp4(Mp4Spec::default())
}

pub async fn upload_gif(
    server: &Server,
    token: &str,
    room_id: &str,
    bytes: Vec<u8>,
) -> (StatusCode, Value) {
    let response = server
        .client
        .post(server.url(&format!("/api/chats/{room_id}/gif-messages/upload")))
        .bearer_auth(token)
        .header("content-type", "application/octet-stream")
        .body(bytes)
        .send()
        .await
        .unwrap();
    let status = response.status();
    (status, response.json().await.unwrap_or(Value::Null))
}

pub async fn send_gif(
    server: &Server,
    token: &str,
    room_id: &str,
    body: Value,
) -> (StatusCode, Value) {
    let response = server
        .client
        .post(server.url(&format!("/api/chats/{room_id}/gif-messages")))
        .bearer_auth(token)
        .json(&body)
        .send()
        .await
        .unwrap();
    let status = response.status();
    (status, response.json().await.unwrap_or(Value::Null))
}

pub async fn save_gif(server: &Server, token: &str, message_id: &Value) -> (StatusCode, Value) {
    let response = server
        .client
        .post(server.url("/api/gifs/saved"))
        .bearer_auth(token)
        .json(&json!({ "message_id": message_id }))
        .send()
        .await
        .unwrap();
    let status = response.status();
    (status, response.json().await.unwrap_or(Value::Null))
}

pub async fn get_json(server: &Server, token: &str, path: &str) -> (StatusCode, Value) {
    let response = server
        .client
        .get(server.url(path))
        .bearer_auth(token)
        .send()
        .await
        .unwrap();
    let status = response.status();
    (status, response.json().await.unwrap_or(Value::Null))
}

pub async fn delete(server: &Server, token: &str, path: &str) -> StatusCode {
    server
        .client
        .delete(server.url(path))
        .bearer_auth(token)
        .send()
        .await
        .unwrap()
        .status()
}

/// Join an open group as `token`.
pub async fn join(server: &Server, token: &str, room_id: &str) {
    server
        .client
        .post(server.url(&format!("/api/chats/{room_id}/join-requests")))
        .bearer_auth(token)
        .json(&json!({}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
}
