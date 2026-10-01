//! TG-1302: image thumbnails are a projection of the original and are authorised exactly like
//! it — the same capability key, gone with the original (recall), never for non-images.

use std::io::Cursor;
use std::sync::Arc;

use chat_room::{build_app, config::AppConfig, state::AppState};
use futures_util::{SinkExt, StreamExt};
use image::{DynamicImage, ImageFormat, Rgb, RgbImage};
use reqwest::{header, multipart};
use tokio::net::TcpListener;
use tokio_tungstenite::{connect_async, tungstenite::Message};

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
    let state = Arc::new(
        AppState::new_with_config(&AppConfig::default())
            .await
            .unwrap(),
    );
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        axum::serve(listener, build_app(state)).await.unwrap();
    });
    TestServer {
        base: format!("http://{address}"),
        task,
    }
}

async fn create_chat(base: &str, token: &str, name: &str) -> String {
    reqwest::Client::new()
        .post(format!("{base}/api/chats"))
        .bearer_auth(token)
        .json(&serde_json::json!({ "name": name, "password": "", "join_policy": "open" }))
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string()
}

fn jpeg(width: u32, height: u32) -> Vec<u8> {
    let mut bytes = Vec::new();
    DynamicImage::ImageRgb8(RgbImage::from_pixel(width, height, Rgb([30, 120, 220])))
        .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Jpeg)
        .unwrap();
    bytes
}

async fn upload(
    base: &str,
    token: &str,
    chat: &str,
    bytes: Vec<u8>,
    name: &str,
    mime: &str,
) -> serde_json::Value {
    let part = multipart::Part::bytes(bytes)
        .file_name(name.to_string())
        .mime_str(mime)
        .unwrap();
    let response = reqwest::Client::new()
        .post(format!("{base}/api/chats/{chat}/attachments"))
        .bearer_auth(token)
        .multipart(multipart::Form::new().part("file", part))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 201);
    response.json().await.unwrap()
}

async fn get(url: String) -> reqwest::Response {
    reqwest::Client::new().get(url).send().await.unwrap()
}

#[tokio::test]
async fn a_photo_gets_a_small_thumbnail_under_its_own_key() {
    let server = start_server().await;
    let token = session_token(&server.base, "thumb-owner").await;
    let chat = create_chat(&server.base, &token, "thumbs").await;
    let source = jpeg(2000, 1500);
    let message = upload(
        &server.base,
        &token,
        &chat,
        source.clone(),
        "big.jpg",
        "image/jpeg",
    )
    .await;
    let attachment = &message["attachment"];
    let id = attachment["id"].as_str().unwrap();
    let download = attachment["download_url"].as_str().unwrap();
    let key = download.split("key=").nth(1).unwrap();
    let thumbnail = attachment["thumbnail_url"].as_str().unwrap();
    assert_eq!(
        thumbnail,
        format!("/api/attachments/{id}/thumbnail?key={key}")
    );

    for round in ["rendered", "cached"] {
        let response = get(format!("{}{thumbnail}", server.base)).await;
        assert_eq!(response.status(), 200, "{round}");
        assert_eq!(response.headers()[header::CONTENT_TYPE], "image/jpeg");
        assert_eq!(response.headers()["x-content-type-options"], "nosniff");
        let bytes = response.bytes().await.unwrap();
        assert!(
            bytes.len() * 4 < source.len(),
            "{round}: {} bytes",
            bytes.len()
        );
        let small = image::load_from_memory(&bytes).unwrap();
        assert_eq!((small.width(), small.height()), (640, 480), "{round}");
    }

    // History carries the same field, so a reload shows thumbnails too.
    let history: Vec<serde_json::Value> = reqwest::Client::new()
        .get(format!("{}/api/chats/{chat}/messages", server.base))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(history[0]["attachment"]["thumbnail_url"], thumbnail);
}

#[tokio::test]
async fn the_thumbnail_is_refused_without_the_originals_key() {
    let server = start_server().await;
    let token = session_token(&server.base, "thumb-keys").await;
    let chat = create_chat(&server.base, &token, "thumb-keys").await;
    let first = upload(
        &server.base,
        &token,
        &chat,
        jpeg(900, 900),
        "a.jpg",
        "image/jpeg",
    )
    .await;
    let second = upload(
        &server.base,
        &token,
        &chat,
        jpeg(900, 900),
        "b.jpg",
        "image/jpeg",
    )
    .await;
    let first_id = first["attachment"]["id"].as_str().unwrap();
    let second_url = second["attachment"]["thumbnail_url"].as_str().unwrap();
    let second_key = second_url.split("key=").nth(1).unwrap();

    // Someone who is not in the chat has no key: a guessed one, another attachment's key, or a
    // made-up id all look exactly like "no such thumbnail".
    let refused = [
        format!(
            "/api/attachments/{first_id}/thumbnail?key={}",
            uuid::Uuid::new_v4()
        ),
        format!("/api/attachments/{first_id}/thumbnail?key={second_key}"),
        format!(
            "/api/attachments/{}/thumbnail?key={second_key}",
            uuid::Uuid::new_v4()
        ),
    ];
    for url in refused {
        assert_eq!(
            get(format!("{}{url}", server.base)).await.status(),
            404,
            "{url}"
        );
    }
    assert_eq!(
        get(format!(
            "{}/api/attachments/{first_id}/thumbnail",
            server.base
        ))
        .await
        .status(),
        400,
        "no key at all"
    );
}

#[tokio::test]
async fn non_images_and_undecodable_images_have_no_thumbnail() {
    let server = start_server().await;
    let token = session_token(&server.base, "thumb-kinds").await;
    let chat = create_chat(&server.base, &token, "thumb-kinds").await;

    let text = upload(
        &server.base,
        &token,
        &chat,
        b"hello".to_vec(),
        "a.txt",
        "text/plain",
    )
    .await;
    assert!(text["attachment"].get("thumbnail_url").is_none(), "{text}");
    let download = text["attachment"]["download_url"].as_str().unwrap();
    let forced = download.replacen("?key=", "/thumbnail?key=", 1);
    assert_eq!(get(format!("{}{forced}", server.base)).await.status(), 404);

    let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" width="9" height="9"/>"#.to_vec();
    let svg = upload(&server.base, &token, &chat, svg, "a.svg", "image/svg+xml").await;
    assert!(svg["attachment"].get("thumbnail_url").is_none(), "{svg}");

    // Declared a PNG, isn't one: offered (the type is an image) but 404 — the client then
    // falls back to the original.
    let fake = upload(
        &server.base,
        &token,
        &chat,
        b"not-a-png".to_vec(),
        "x.png",
        "image/png",
    )
    .await;
    let url = fake["attachment"]["thumbnail_url"].as_str().unwrap();
    assert_eq!(get(format!("{}{url}", server.base)).await.status(), 404);
}

#[tokio::test]
async fn a_recalled_photos_thumbnail_is_gone_even_when_cached() {
    let server = start_server().await;
    let token = session_token(&server.base, "thumb-recall").await;
    let chat = create_chat(&server.base, &token, "thumb-recall").await;
    let message = upload(
        &server.base,
        &token,
        &chat,
        jpeg(1200, 800),
        "r.jpg",
        "image/jpeg",
    )
    .await;
    let url = format!(
        "{}{}",
        server.base,
        message["attachment"]["thumbnail_url"].as_str().unwrap()
    );
    assert_eq!(get(url.clone()).await.status(), 200, "rendered and cached");

    let ws = format!("{}/ws/{chat}", server.base.replacen("http://", "ws://", 1));
    let (mut socket, _) = connect_async(ws).await.unwrap();
    let send = |value: serde_json::Value| Message::Text(value.to_string());
    socket
        .send(send(serde_json::json!({ "type": "join", "token": token })))
        .await
        .unwrap();
    socket
        .send(send(
            serde_json::json!({ "type": "recall", "message_id": message["id"] }),
        ))
        .await
        .unwrap();
    loop {
        let Message::Text(text) = socket.next().await.unwrap().unwrap() else {
            continue;
        };
        if serde_json::from_str::<serde_json::Value>(&text).unwrap()["type"] == "message_recalled" {
            break;
        }
    }
    assert_eq!(get(url).await.status(), 404);
}
