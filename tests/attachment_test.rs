use std::sync::Arc;

use chat_room::{
    build_app,
    config::{AppConfig, UploadConfig},
    state::AppState,
};
use futures_util::{SinkExt, StreamExt};
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
    start_server_with_config(AppConfig::default()).await
}

async fn start_server_with_config(config: AppConfig) -> TestServer {
    let state = Arc::new(AppState::new_with_config(&config).await.unwrap());
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

#[tokio::test]
async fn configured_upload_limit_is_public_and_enforced() {
    let server = start_server_with_config(AppConfig {
        uploads: UploadConfig {
            max_file_size_mib: 1,
        },
        ..AppConfig::default()
    })
    .await;
    let client = reqwest::Client::new();
    let config: serde_json::Value = client
        .get(format!("{}/api/config", server.base))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(config["max_upload_bytes"], 1024 * 1024);

    let room_id = create_room(&server.base, "limited-media", "").await;
    let token = session_token(&server.base, "limited-uploader").await;
    let mut socket = connect_room(&server.base, &room_id, &token).await;
    let response = client
        .post(format!("{}/api/rooms/{room_id}/attachments", server.base))
        .bearer_auth(token)
        .multipart(upload_form(
            vec![0; 1024 * 1024 + 1],
            "too-large.bin",
            "application/octet-stream",
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 413);
    socket.close(None).await.unwrap();
}

async fn create_room(base: &str, name: &str, password: &str) -> String {
    let owner_token = session_token(base, &format!("{name}-owner")).await;
    reqwest::Client::new()
        .post(format!("{base}/api/rooms"))
        .bearer_auth(owner_token)
        .json(&serde_json::json!({ "name": name, "password": password, "join_policy": "open" }))
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

async fn connect_room(
    base: &str,
    room_id: &str,
    token: &str,
) -> tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>> {
    let url = format!("{}/ws/{room_id}", base.replacen("http://", "ws://", 1));
    let (mut socket, _) = connect_async(url).await.unwrap();
    socket
        .send(Message::Text(
            serde_json::json!({ "type": "join", "token": token }).to_string(),
        ))
        .await
        .unwrap();
    assert_eq!(next_json(&mut socket).await["type"], "auth_ok");
    assert_eq!(next_json(&mut socket).await["type"], "system");
    socket
}

async fn next_json(
    socket: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
) -> serde_json::Value {
    loop {
        let frame = socket.next().await.unwrap().unwrap();
        if let Message::Text(text) = frame {
            return serde_json::from_str(&text).unwrap();
        }
    }
}

fn upload_form(bytes: Vec<u8>, name: &str, mime: &str) -> multipart::Form {
    let part = multipart::Part::bytes(bytes)
        .file_name(name.to_string())
        .mime_str(mime)
        .unwrap();
    multipart::Form::new().part("file", part)
}

#[tokio::test]
async fn attachment_upload_replays_and_supports_range_downloads() {
    let server = start_server().await;
    let client = reqwest::Client::new();
    let room_id = create_room(&server.base, "media", "").await;
    let token = session_token(&server.base, "alice-media").await;
    let mut socket = connect_room(&server.base, &room_id, &token).await;
    let bytes = b"fake-png-binary".to_vec();

    let response = client
        .post(format!("{}/api/rooms/{room_id}/attachments", server.base))
        .bearer_auth(&token)
        .multipart(
            upload_form(bytes.clone(), "tiny.png", "image/png")
                .text("content", "screenshot caption"),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 201);
    let uploaded: serde_json::Value = response.json().await.unwrap();
    assert_eq!(uploaded["sender"], "alice-media");
    assert_eq!(uploaded["content"], "screenshot caption");
    assert_eq!(uploaded["attachment"]["file_name"], "tiny.png");
    assert_eq!(uploaded["attachment"]["mime_type"], "image/png");
    assert_eq!(uploaded["attachment"]["size_bytes"], bytes.len());
    let download_url = uploaded["attachment"]["download_url"]
        .as_str()
        .unwrap()
        .to_string();

    let broadcast = next_json(&mut socket).await;
    assert_eq!(broadcast["type"], "broadcast");
    assert_eq!(broadcast["message_id"], uploaded["id"]);
    assert_eq!(broadcast["attachment"], uploaded["attachment"]);

    let full = client
        .get(format!("{}{}", server.base, download_url))
        .send()
        .await
        .unwrap();
    assert_eq!(full.status(), 200);
    assert_eq!(full.headers()[header::CONTENT_TYPE], "image/png");
    assert_eq!(full.headers()[header::ACCEPT_RANGES], "bytes");
    assert_eq!(full.bytes().await.unwrap().as_ref(), bytes.as_slice());

    let partial = client
        .get(format!("{}{}", server.base, download_url))
        .header(header::RANGE, "bytes=2-5")
        .send()
        .await
        .unwrap();
    assert_eq!(partial.status(), 206);
    assert_eq!(
        partial.headers()[header::CONTENT_RANGE],
        format!("bytes 2-5/{}", bytes.len())
    );
    assert_eq!(partial.bytes().await.unwrap().as_ref(), &bytes[2..=5]);

    let history = client
        .get(format!("{}/api/rooms/{room_id}/messages", server.base))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap()
        .json::<Vec<serde_json::Value>>()
        .await
        .unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0]["attachment"], uploaded["attachment"]);
    assert_eq!(history[0]["content"], "screenshot caption");

    socket
        .send(Message::Text(
            serde_json::json!({ "type": "recall", "message_id": uploaded["id"] }).to_string(),
        ))
        .await
        .unwrap();
    assert_eq!(next_json(&mut socket).await["type"], "message_recalled");
    assert_eq!(
        client
            .get(format!("{}{}", server.base, download_url))
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
}

#[tokio::test]
async fn private_room_upload_requires_account_and_room_credentials() {
    let server = start_server().await;
    let client = reqwest::Client::new();
    let room_id = create_room(&server.base, "private-media", "room-secret").await;
    let token = session_token(&server.base, "private-media-user").await;
    let url = format!("{}/api/rooms/{room_id}/attachments", server.base);

    let anonymous = client
        .post(&url)
        .header("x-room-password", "room-secret")
        .multipart(upload_form(vec![1], "file.bin", "application/octet-stream"))
        .send()
        .await
        .unwrap();
    assert_eq!(anonymous.status(), 401);

    let wrong_password = client
        .post(&url)
        .bearer_auth(&token)
        .header("x-room-password", "wrong")
        .multipart(upload_form(vec![1], "file.bin", "application/octet-stream"))
        .send()
        .await
        .unwrap();
    assert_eq!(wrong_password.status(), 401);

    let accepted = client
        .post(format!("{}/api/rooms/{room_id}/join-requests", server.base))
        .bearer_auth(&token)
        .json(&serde_json::json!({ "password": "room-secret" }))
        .send()
        .await
        .unwrap();
    assert_eq!(accepted.status(), 200);

    let accepted = client
        .post(&url)
        .bearer_auth(&token)
        .header("x-room-password", "room-secret")
        .multipart(upload_form(
            vec![1, 2, 3],
            "file.bin",
            "application/octet-stream",
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(accepted.status(), 201);
    let message: serde_json::Value = accepted.json().await.unwrap();
    let mut invalid_url = message["attachment"]["download_url"]
        .as_str()
        .unwrap()
        .to_string();
    let replacement = if invalid_url.ends_with('0') { "1" } else { "0" };
    invalid_url.replace_range(invalid_url.len() - 1.., replacement);
    assert_eq!(
        client
            .get(format!("{}{}", server.base, invalid_url))
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
}
