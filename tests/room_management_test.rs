use chat_room::{build_app, state::AppState};
use futures_util::{SinkExt, StreamExt};
use std::{path::Path, sync::Arc};
use tokio::{net::TcpListener, task::JoinHandle, time::Duration};
use tokio_tungstenite::{connect_async, tungstenite::Message};

mod support;
use support::session_token;

async fn start_server() -> (String, Arc<AppState>, JoinHandle<()>) {
    let state = Arc::new(AppState::new().await.unwrap());
    start_server_with_state(state).await
}

async fn start_server_with_state(state: Arc<AppState>) -> (String, Arc<AppState>, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let app = build_app(state.clone());
    let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (format!("http://{address}"), state, task)
}

fn remove_sqlite_files(path: &Path) {
    let _ = std::fs::remove_file(path);
    let _ = std::fs::remove_file(format!("{}-wal", path.display()));
    let _ = std::fs::remove_file(format!("{}-shm", path.display()));
}

async fn create_room(base: &str, name: &str, password: &str) -> serde_json::Value {
    let owner_token = session_token(base, &format!("owner-{name}")).await;
    let response = reqwest::Client::new()
        .post(format!("{base}/api/rooms"))
        .bearer_auth(&owner_token)
        .json(&serde_json::json!({
            "name": name,
            "password": password,
            "join_policy": "open"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 201);
    let mut room: serde_json::Value = response.json().await.unwrap();
    room["_test_owner_token"] = owner_token.into();
    room
}

async fn connect_room(
    base: &str,
    room_id: &str,
    password: Option<&str>,
) -> tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>> {
    let ws_url = format!("{}/ws/{room_id}", base.replace("http://", "ws://"));
    let (mut socket, _) = connect_async(ws_url).await.unwrap();
    let token = session_token(base, "tester").await;
    let greeting = match password {
        Some(password) => serde_json::json!({
            "type": "auth",
            "token": token,
            "password": password
        }),
        None => serde_json::json!({ "type": "join", "token": token }),
    };
    socket
        .send(Message::Text(greeting.to_string()))
        .await
        .unwrap();
    socket
}

async fn next_json(
    socket: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
) -> serde_json::Value {
    loop {
        let frame = tokio::time::timeout(Duration::from_secs(2), socket.next())
            .await
            .expect("timed out waiting for WebSocket frame")
            .expect("WebSocket ended")
            .expect("WebSocket error");
        let Message::Text(text) = frame else { continue };
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        if value["type"] != "history_complete" {
            return value;
        }
    }
}

async fn next_content(
    socket: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
    expected: &str,
) -> serde_json::Value {
    loop {
        let message = next_json(socket).await;
        if message["content"] == expected {
            return message;
        }
    }
}

#[tokio::test]
async fn public_room_can_be_renamed_and_deleted() {
    let (base, _state, task) = start_server().await;
    let first = create_room(&base, "first", "").await;
    let owner_token = first["_test_owner_token"].as_str().unwrap();
    create_room(&base, "taken", "").await;
    let room_url = format!("{}/api/rooms/{}", base, first["id"].as_str().unwrap());
    let client = reqwest::Client::new();

    let conflict = client
        .patch(&room_url)
        .bearer_auth(owner_token)
        .json(&serde_json::json!({ "name": "taken" }))
        .send()
        .await
        .unwrap();
    assert_eq!(conflict.status(), 409);

    let updated: serde_json::Value = client
        .patch(&room_url)
        .bearer_auth(owner_token)
        .json(&serde_json::json!({ "name": "renamed" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(updated["name"], "renamed");
    assert_eq!(
        client
            .delete(&room_url)
            .bearer_auth(owner_token)
            .send()
            .await
            .unwrap()
            .status(),
        204
    );
    assert_eq!(client.get(&room_url).send().await.unwrap().status(), 404);
    task.abort();
}

#[tokio::test]
async fn private_room_management_requires_current_password() {
    let (base, _state, task) = start_server().await;
    let room = create_room(&base, "private", "old-secret").await;
    let room_id = room["id"].as_str().unwrap();
    let owner_token = room["_test_owner_token"].as_str().unwrap();
    let room_url = format!("{base}/api/rooms/{room_id}");
    let client = reqwest::Client::new();

    let unauthorized = client
        .patch(&room_url)
        .json(&serde_json::json!({
            "name": "not-renamed",
            "current_password": "wrong"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(unauthorized.status(), 401);

    let mut old_session = connect_room(&base, room_id, Some("old-secret")).await;
    assert_eq!(next_json(&mut old_session).await["type"], "auth_ok");
    let updated = client
        .patch(&room_url)
        .bearer_auth(owner_token)
        .json(&serde_json::json!({
            "name": "renamed-private",
            "current_password": "old-secret",
            "new_password": "new-secret"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(updated.status(), 200);

    let mut saw_disconnect_reason = false;
    for _ in 0..3 {
        let message = next_json(&mut old_session).await;
        if message["content"] == "room password changed" {
            saw_disconnect_reason = true;
            break;
        }
    }
    assert!(saw_disconnect_reason);

    let mut rejected = connect_room(&base, room_id, Some("old-secret")).await;
    assert_eq!(next_json(&mut rejected).await["type"], "auth_fail");
    let mut accepted = connect_room(&base, room_id, Some("new-secret")).await;
    assert_eq!(next_json(&mut accepted).await["type"], "auth_ok");

    let made_public: serde_json::Value = client
        .patch(&room_url)
        .bearer_auth(owner_token)
        .json(&serde_json::json!({
            "current_password": "new-secret",
            "new_password": ""
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(made_public["has_password"], false);
    let mut public_session = connect_room(&base, room_id, None).await;
    assert_eq!(next_json(&mut public_session).await["type"], "auth_ok");
    task.abort();
}

#[tokio::test]
async fn deleting_private_room_cascades_messages_and_disconnects_members() {
    let (base, state, task) = start_server().await;
    let room = create_room(&base, "temporary", "secret").await;
    let room_id = room["id"].as_str().unwrap();
    let owner_token = room["_test_owner_token"].as_str().unwrap();
    let room_url = format!("{base}/api/rooms/{room_id}");
    let mut socket = connect_room(&base, room_id, Some("secret")).await;
    assert_eq!(next_json(&mut socket).await["type"], "auth_ok");
    assert_eq!(next_json(&mut socket).await["type"], "system");
    socket
        .send(Message::Text(
            serde_json::json!({ "type": "message", "content": "persisted" }).to_string(),
        ))
        .await
        .unwrap();
    assert_eq!(
        next_content(&mut socket, "persisted").await["type"],
        "broadcast"
    );

    let client = reqwest::Client::new();
    assert_eq!(client.delete(&room_url).send().await.unwrap().status(), 401);
    assert_eq!(
        client
            .delete(&room_url)
            .bearer_auth(owner_token)
            .send()
            .await
            .unwrap()
            .status(),
        204
    );
    assert_eq!(
        next_content(&mut socket, "room deleted").await["content"],
        "room deleted"
    );

    let stored: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM messages WHERE room_id = ?")
        .bind(room_id)
        .fetch_one(state.pool())
        .await
        .unwrap();
    assert_eq!(stored, 0);
    assert_eq!(client.get(&room_url).send().await.unwrap().status(), 404);
    task.abort();
}

#[tokio::test]
async fn messages_reach_websockets_connected_to_different_server_processes() {
    let database = std::env::temp_dir().join(format!(
        "chat-room-cross-process-{}.db",
        uuid::Uuid::new_v4()
    ));
    let state_a = Arc::new(AppState::open(&database).await.unwrap());
    let (base_a, state_a, task_a) = start_server_with_state(state_a).await;
    let room = create_room(&base_a, "shared-room", "").await;
    let room_id = room["id"].as_str().unwrap();

    // A second AppState models another Rust process sharing the same SQLite file.
    let state_b = Arc::new(AppState::open(&database).await.unwrap());
    let (base_b, state_b, task_b) = start_server_with_state(state_b).await;
    let mut socket_a = connect_room(&base_a, room_id, None).await;
    let mut socket_b = connect_room(&base_b, room_id, None).await;
    assert_eq!(next_json(&mut socket_a).await["type"], "auth_ok");
    assert_eq!(next_json(&mut socket_b).await["type"], "auth_ok");
    assert_eq!(next_json(&mut socket_a).await["type"], "system");
    assert_eq!(next_json(&mut socket_b).await["type"], "presence");

    socket_a
        .send(Message::Text(
            serde_json::json!({ "type": "message", "content": "cross-process" }).to_string(),
        ))
        .await
        .unwrap();
    let received = next_json(&mut socket_b).await;
    assert_eq!(received["type"], "broadcast");
    assert_eq!(received["content"], "cross-process");

    drop(socket_a);
    drop(socket_b);
    task_a.abort();
    task_b.abort();
    state_a.pool().close().await;
    state_b.pool().close().await;
    remove_sqlite_files(&database);
}
