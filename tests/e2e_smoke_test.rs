//! Black-box release smoke test across HTTP, WebSocket, upload, search, and AI status.

use std::{sync::Arc, time::Duration};

use chat_room::{build_app, config::AppConfig, state::AppState};
use futures_util::{SinkExt, StreamExt};
use reqwest::{multipart, Client, StatusCode};
use serde_json::{json, Value};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::{connect_async, tungstenite::Message, MaybeTlsStream, WebSocketStream};

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

struct TestServer {
    base: String,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn start_server(config: AppConfig) -> TestServer {
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

async fn auth(client: &Client, base: &str, action: &str, username: &str) -> (StatusCode, Value) {
    let response = client
        .post(format!("{base}/api/users/{action}"))
        .json(&json!({ "username": username, "password": "e2e-password" }))
        .send()
        .await
        .unwrap();
    let status = response.status();
    let body = response.json().await.unwrap();
    (status, body)
}

async fn connect_room(base: &str, room_id: &str, token: &str) -> Socket {
    let url = format!("{}/ws/{room_id}", base.replacen("http://", "ws://", 1));
    let (mut socket, _) = connect_async(url).await.unwrap();
    socket
        .send(Message::Text(
            json!({ "type": "join", "token": token }).to_string(),
        ))
        .await
        .unwrap();
    assert_eq!(next_event(&mut socket, "auth_ok").await["type"], "auth_ok");
    socket
}

async fn next_event(socket: &mut Socket, expected_type: &str) -> Value {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let frame = socket.next().await.unwrap().unwrap();
            if let Message::Text(text) = frame {
                let event: Value = serde_json::from_str(&text).unwrap();
                if event["type"] == expected_type {
                    return event;
                }
            }
        }
    })
    .await
    .unwrap_or_else(|_| panic!("timed out waiting for WebSocket event {expected_type}"))
}

#[tokio::test]
async fn release_smoke_covers_account_room_message_upload_and_search() {
    let server = start_server(AppConfig::default()).await;
    let client = Client::new();

    let (status, _) = auth(&client, &server.base, "register", "e2e-alice").await;
    assert_eq!(status, StatusCode::CREATED);
    let (status, alice) = auth(&client, &server.base, "login", "e2e-alice").await;
    assert_eq!(status, StatusCode::OK);
    let alice_token = alice["token"].as_str().unwrap();

    let (status, bob) = auth(&client, &server.base, "register", "e2e-bob").await;
    assert_eq!(status, StatusCode::CREATED);
    let bob_token = bob["token"].as_str().unwrap();

    let response = client
        .post(format!("{}/api/rooms", server.base))
        .bearer_auth(alice_token)
        .json(&json!({
            "name": "FND-004 release room",
            "password": "",
            "join_policy": "open"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let room: Value = response.json().await.unwrap();
    let room_id = room["id"].as_str().unwrap();

    let mut alice_socket = connect_room(&server.base, room_id, alice_token).await;
    let mut bob_socket = connect_room(&server.base, room_id, bob_token).await;
    alice_socket
        .send(Message::Text(
            json!({
                "type": "message",
                "content": "FND-004 e2e searchable message",
                "client_message_id": uuid::Uuid::new_v4()
            })
            .to_string(),
        ))
        .await
        .unwrap();
    let broadcast = next_event(&mut bob_socket, "broadcast").await;
    assert_eq!(broadcast["sender"], "e2e-alice");
    assert_eq!(broadcast["content"], "FND-004 e2e searchable message");

    let file = multipart::Part::bytes(b"release-smoke-file".to_vec())
        .file_name("release-smoke.txt")
        .mime_str("text/plain")
        .unwrap();
    let upload = client
        .post(format!("{}/api/rooms/{room_id}/attachments", server.base))
        .bearer_auth(bob_token)
        .multipart(
            multipart::Form::new()
                .part("file", file)
                .text("content", "FND-004 upload caption"),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(upload.status(), StatusCode::CREATED);
    let uploaded: Value = upload.json().await.unwrap();
    assert_eq!(uploaded["attachment"]["file_name"], "release-smoke.txt");

    let search = client
        .get(format!(
            "{}/api/rooms/{room_id}/messages/search",
            server.base
        ))
        .bearer_auth(bob_token)
        .query(&[("q", "FND-004"), ("limit", "20")])
        .send()
        .await
        .unwrap();
    assert_eq!(search.status(), StatusCode::OK);
    let matches: Vec<Value> = search.json().await.unwrap();
    assert_eq!(matches.len(), 2);
    assert!(matches
        .iter()
        .any(|message| message["content"] == "FND-004 e2e searchable message"));
    assert!(matches
        .iter()
        .any(|message| message["attachment"]["file_name"] == "release-smoke.txt"));
}

#[tokio::test]
async fn public_config_reports_ai_disabled_and_ready_states() {
    let disabled_server = start_server(AppConfig::default()).await;
    let client = Client::new();
    let disabled: Value = client
        .get(format!("{}/api/config", disabled_server.base))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(disabled["ai_status"], "disabled");
    assert_eq!(disabled["ai_enabled"], false);

    let credential_variable = format!("CHAT_ROOM_E2E_AI_KEY_{}", uuid::Uuid::new_v4().simple());
    std::env::set_var(&credential_variable, "release-smoke-key");
    let mut ready_config = AppConfig::default();
    ready_config.ai.enabled = true;
    ready_config.ai.api_key_env = credential_variable.clone();
    ready_config.ai.model = "release-smoke-model".into();
    let ready_server = start_server(ready_config).await;
    let ready: Value = client
        .get(format!("{}/api/config", ready_server.base))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(ready["ai_status"], "ready");
    assert_eq!(ready["ai_enabled"], true);
    std::env::remove_var(credential_variable);
}
