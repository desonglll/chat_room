//! Integration tests — REST API, WebSocket (public + private), and persistence.

use chat_room::{build_app, state::AppState};
use futures_util::{SinkExt, StreamExt};
use sha2::{Digest, Sha256};
use std::{
    fmt,
    ops::Deref,
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::{net::TcpListener, task::JoinHandle};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

struct TestServer {
    base: String,
    task: Option<JoinHandle<()>>,
}

impl Deref for TestServer {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl fmt::Display for TestServer {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.base.fmt(formatter)
    }
}

impl TestServer {
    async fn shutdown(mut self) {
        if let Some(task) = self.task.take() {
            task.abort();
            let _ = task.await;
        }
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

async fn start_server() -> TestServer {
    let state = Arc::new(AppState::new().await.unwrap());
    start_server_with_state(state).await
}

async fn start_server_with_state(state: Arc<AppState>) -> TestServer {
    let app = build_app(state);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let task = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    TestServer {
        base: format!("http://127.0.0.1:{}", port),
        task: Some(task),
    }
}

fn temp_path(prefix: &str, extension: &str) -> PathBuf {
    std::env::temp_dir().join(format!("{}-{}.{}", prefix, uuid::Uuid::new_v4(), extension))
}

fn remove_sqlite_files(path: &Path) {
    let _ = std::fs::remove_file(path);
    let _ = std::fs::remove_file(format!("{}-wal", path.display()));
    let _ = std::fs::remove_file(format!("{}-shm", path.display()));
}

/// Create a room, return its UUID and whether it has_password.
async fn create_room(base: &str, name: &str, password: Option<&str>) -> (String, bool) {
    let client = reqwest::Client::new();
    let body = serde_json::json!({
        "name": name,
        "password": password.unwrap_or("")
    });
    let resp = client
        .post(format!("{}/api/rooms", base))
        .json(&body)
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        201,
        "create_room failed: {:?}",
        resp.text().await
    );
    let body: serde_json::Value = resp.json().await.unwrap();
    let id = body["id"].as_str().unwrap().to_string();
    let has_password = body["has_password"].as_bool().unwrap_or(false);
    (id, has_password)
}

/// Read the next text frame from a WebSocket stream and parse as JSON.
async fn read_json(
    stream: &mut futures_util::stream::SplitStream<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
    >,
) -> serde_json::Value {
    match stream.next().await {
        Some(Ok(Message::Text(t))) => serde_json::from_str::<serde_json::Value>(&t).unwrap(),
        other => panic!("expected text frame, got {:?}", other),
    }
}

/// Connect via WebSocket and authenticate (private room) or join (public room).
/// Returns (sink, stream).
async fn ws_connect(
    base: &str,
    room_id: &str,
    username: &str,
    password: Option<&str>,
) -> (
    futures_util::stream::SplitSink<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        Message,
    >,
    futures_util::stream::SplitStream<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
    >,
) {
    let ws_base = base.replace("http://", "ws://");
    let url = format!("{}/ws/{}", ws_base, room_id);
    let (ws, _) = connect_async(&url).await.unwrap();
    let (mut sink, mut stream) = ws.split();

    let first = if let Some(pw) = password {
        serde_json::json!({ "type": "auth", "username": username, "password": pw })
    } else {
        serde_json::json!({ "type": "join", "username": username })
    };
    sink.send(Message::Text(first.to_string())).await.unwrap();

    // Read auth_ok response.
    let raw = match stream.next().await {
        Some(Ok(Message::Text(t))) => t.to_string(),
        other => panic!("expected auth response, got {:?}", other),
    };
    let resp: serde_json::Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(resp["type"], "auth_ok", "auth/join failed: {}", resp);

    (sink, stream)
}

// ── REST API tests ──────────────────────────────────────────────────────────

#[tokio::test]
async fn create_and_list_rooms() {
    let base = start_server().await;

    let list: Vec<serde_json::Value> = reqwest::get(format!("{}/api/rooms", base))
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(list.is_empty());

    let id1 = create_room(&base, "general", Some("pw1")).await.0;
    let id2 = create_room(&base, "random", None).await.0;
    assert_ne!(id1, id2);

    let list: Vec<serde_json::Value> = reqwest::get(format!("{}/api/rooms", base))
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(list.len(), 2);
}

#[tokio::test]
async fn public_room_has_password_false() {
    let base = start_server().await;
    let (id, has_password) = create_room(&base, "lobby", None).await;
    assert!(!has_password, "public room should have has_password=false");

    let resp = reqwest::get(format!("{}/api/rooms/{}", base, id))
        .await
        .unwrap();
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(!body["has_password"].as_bool().unwrap());
}

#[tokio::test]
async fn private_room_has_password_true() {
    let base = start_server().await;
    let (id, has_password) = create_room(&base, "vip", Some("secret")).await;
    assert!(has_password);

    let resp = reqwest::get(format!("{}/api/rooms/{}", base, id))
        .await
        .unwrap();
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body["has_password"].as_bool().unwrap());
}

#[tokio::test]
async fn reject_duplicate_room_name() {
    let base = start_server().await;
    create_room(&base, "lobby", None).await;

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{}/api/rooms", base))
        .json(&serde_json::json!({ "name": "lobby", "password": "" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 409);
}

#[tokio::test]
async fn get_room_by_id() {
    let base = start_server().await;
    let (id, _) = create_room(&base, "mychat", Some("secret")).await;

    let resp = reqwest::get(format!("{}/api/rooms/{}", base, id))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["name"], "mychat");
    assert!(body.get("password_hash").is_none());

    let resp = reqwest::get(format!(
        "{}/api/rooms/00000000-0000-0000-0000-000000000000",
        base
    ))
    .await
    .unwrap();
    assert_eq!(resp.status(), 404);
}

// ── WebSocket tests — private rooms ─────────────────────────────────────────

#[tokio::test]
async fn ws_auth_wrong_password() {
    let base = start_server().await;
    let (id, _) = create_room(&base, "secret-room", Some("correct")).await;

    let ws_base = base.replace("http://", "ws://");
    let url = format!("{}/ws/{}", ws_base, id);
    let (ws, _) = connect_async(&url).await.unwrap();
    let (mut sink, mut stream) = ws.split();

    let auth = serde_json::json!({ "type": "auth", "username": "alice", "password": "wrong" });
    sink.send(Message::Text(auth.to_string())).await.unwrap();

    let raw = match stream.next().await {
        Some(Ok(Message::Text(t))) => t.to_string(),
        other => panic!("expected auth_fail, got {:?}", other),
    };
    let resp: serde_json::Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(resp["type"], "auth_fail");
    assert!(resp["reason"].as_str().unwrap().contains("password"));
}

#[tokio::test]
async fn ws_private_join_and_chat() {
    let base = start_server().await;
    let (id, _) = create_room(&base, "private-chat", Some("pw")).await;

    let (mut sink_a, mut stream_a) = ws_connect(&base, &id, "alice", Some("pw")).await;
    let (_sink_b, mut stream_b) = ws_connect(&base, &id, "bob", Some("pw")).await;

    // Alice: own join, then Bob's join.
    let msg = read_json(&mut stream_a).await;
    assert_eq!(msg["type"], "system");
    assert!(msg["content"].as_str().unwrap().contains("alice"));
    let msg = read_json(&mut stream_a).await;
    assert_eq!(msg["type"], "system");
    assert!(msg["content"].as_str().unwrap().contains("bob"));

    // Bob: own join.
    let msg = read_json(&mut stream_b).await;
    assert_eq!(msg["type"], "system");
    assert!(msg["content"].as_str().unwrap().contains("bob"));

    // Alice sends a message.
    sink_a
        .send(Message::Text(
            serde_json::json!({ "type": "message", "content": "Hello Bob!" }).to_string(),
        ))
        .await
        .unwrap();

    // Bob receives, Alice gets echo.
    let msg = read_json(&mut stream_b).await;
    assert_eq!(msg["sender"], "alice");
    assert_eq!(msg["content"], "Hello Bob!");
    let msg = read_json(&mut stream_a).await;
    assert_eq!(msg["sender"], "alice");
    assert_eq!(msg["content"], "Hello Bob!");
}

#[tokio::test]
async fn ws_leave_notifies_others() {
    let base = start_server().await;
    let (id, _) = create_room(&base, "room", Some("pw")).await;

    let (_sink_a, mut stream_a) = ws_connect(&base, &id, "alice", Some("pw")).await;
    let (sink_b, stream_b) = ws_connect(&base, &id, "bob", Some("pw")).await;

    // Alice: own join.
    let msg = read_json(&mut stream_a).await;
    assert!(msg["content"].as_str().unwrap().contains("alice"));
    // Alice: Bob's join.
    let msg = read_json(&mut stream_a).await;
    assert!(msg["content"].as_str().unwrap().contains("bob"));

    drop(sink_b);
    drop(stream_b);

    let msg = read_json(&mut stream_a).await;
    assert_eq!(msg["type"], "system");
    assert!(msg["content"].as_str().unwrap().contains("bob"));
}

#[tokio::test]
async fn ws_nonexistent_room() {
    let base = start_server().await;
    let ws_base = base.replace("http://", "ws://");
    let url = format!("{}/ws/00000000-0000-0000-0000-000000000000", ws_base);
    let (ws, _) = connect_async(&url).await.unwrap();
    let (_, mut stream) = ws.split();

    let raw = match stream.next().await {
        Some(Ok(Message::Text(t))) => t.to_string(),
        other => panic!("expected auth_fail, got {:?}", other),
    };
    let resp: serde_json::Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(resp["type"], "auth_fail");
}

// ── WebSocket tests — public rooms ──────────────────────────────────────────

#[tokio::test]
async fn ws_public_join_no_password() {
    let base = start_server().await;
    let (id, has_password) = create_room(&base, "public-lounge", None).await;
    assert!(!has_password);

    // Join with "join" message (no password).
    let (_sink, _stream) = ws_connect(&base, &id, "guest", None).await;
}

#[tokio::test]
async fn ws_public_join_with_auth_also_works() {
    let base = start_server().await;
    let (id, _) = create_room(&base, "open", None).await;

    // Sending auth to a public room should still work (password ignored).
    let (_sink, _stream) = ws_connect(&base, &id, "alice", Some("anything")).await;
}

#[tokio::test]
async fn ws_public_join_rejects_if_private() {
    let base = start_server().await;
    let (id, _) = create_room(&base, "vip-room", Some("secret")).await;

    let ws_base = base.replace("http://", "ws://");
    let url = format!("{}/ws/{}", ws_base, id);
    let (ws, _) = connect_async(&url).await.unwrap();
    let (mut sink, mut stream) = ws.split();

    // Send "join" to a private room — should be rejected.
    let join = serde_json::json!({ "type": "join", "username": "alice" });
    sink.send(Message::Text(join.to_string())).await.unwrap();

    let raw = match stream.next().await {
        Some(Ok(Message::Text(t))) => t.to_string(),
        other => panic!("expected auth_fail, got {:?}", other),
    };
    let resp: serde_json::Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(resp["type"], "auth_fail");
    assert!(resp["reason"].as_str().unwrap().contains("password"));
}

#[tokio::test]
async fn ws_public_chat_works() {
    let base = start_server().await;
    let (id, _) = create_room(&base, "open-chat", None).await;

    let (mut sink_a, mut stream_a) = ws_connect(&base, &id, "alice", None).await;
    let (_sink_b, mut stream_b) = ws_connect(&base, &id, "bob", None).await;

    // Consume join messages.
    let msg = read_json(&mut stream_a).await;
    assert!(msg["content"].as_str().unwrap().contains("alice"));
    let msg = read_json(&mut stream_a).await;
    assert!(msg["content"].as_str().unwrap().contains("bob"));
    let msg = read_json(&mut stream_b).await;
    assert!(msg["content"].as_str().unwrap().contains("bob"));

    // Chat.
    sink_a
        .send(Message::Text(
            serde_json::json!({ "type": "message", "content": "Hi from public room!" }).to_string(),
        ))
        .await
        .unwrap();

    let msg = read_json(&mut stream_b).await;
    assert_eq!(msg["sender"], "alice");
    assert_eq!(msg["content"], "Hi from public room!");
}

// ── Persistence tests ──────────────────────────────────────────────────────

#[tokio::test]
async fn rooms_survive_sqlite_restart() {
    let database = temp_path("chat-rooms-restart", "db");
    assert!(!database.exists());

    let state1 = Arc::new(AppState::open(&database, None).await.unwrap());
    assert!(
        database.exists(),
        "database should be created automatically"
    );
    let server1 = start_server_with_state(state1.clone()).await;

    let (private_id, _) = create_room(&server1, "persistent-private", Some("pw")).await;
    let (public_id, _) = create_room(&server1, "persistent-public", None).await;

    let stored: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM rooms")
        .fetch_one(state1.pool())
        .await
        .unwrap();
    assert_eq!(stored, 2);

    let journal_mode: String = sqlx::query_scalar("PRAGMA journal_mode")
        .fetch_one(state1.pool())
        .await
        .unwrap();
    assert_eq!(journal_mode, "wal");

    server1.shutdown().await;
    state1.pool().close().await;
    drop(state1);

    let state2 = Arc::new(AppState::open(&database, None).await.unwrap());
    let server2 = start_server_with_state(state2.clone()).await;
    let list: Vec<serde_json::Value> = reqwest::get(format!("{}/api/rooms", server2))
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(list.len(), 2);

    let ids: Vec<&str> = list.iter().filter_map(|room| room["id"].as_str()).collect();
    assert!(ids.contains(&private_id.as_str()));
    assert!(ids.contains(&public_id.as_str()));

    // Both access modes still work after reopening SQLite.
    let (_private_sink, _private_stream) =
        ws_connect(&server2, &private_id, "returning-user", Some("pw")).await;
    let (_public_sink, _public_stream) = ws_connect(&server2, &public_id, "guest", None).await;

    server2.shutdown().await;
    state2.pool().close().await;
    remove_sqlite_files(&database);
}

#[tokio::test]
async fn legacy_json_is_imported_once_without_exposing_private_rooms() {
    let database = temp_path("chat-rooms-legacy", "db");
    let legacy = temp_path("chat-rooms-legacy", "json");
    let public_id = uuid::Uuid::new_v4();
    let private_id = uuid::Uuid::new_v4();
    let broken_private_id = uuid::Uuid::new_v4();

    let mut hasher = Sha256::new();
    hasher.update(b"secret");
    let private_hash = hex::encode(hasher.finalize());
    let mut rooms = serde_json::Map::new();
    rooms.insert(
        public_id.to_string(),
        serde_json::json!({
            "id": public_id,
            "name": "legacy-public",
            "has_password": false,
            "created_at": "2026-08-14T06:00:00Z"
        }),
    );
    rooms.insert(
        private_id.to_string(),
        serde_json::json!({
            "id": private_id,
            "name": "legacy-private",
            "password_hash": private_hash,
            "has_password": true,
            "created_at": "2026-08-14T06:01:00Z"
        }),
    );
    rooms.insert(
        broken_private_id.to_string(),
        serde_json::json!({
            "id": broken_private_id,
            "name": "legacy-private-without-hash",
            "has_password": true,
            "created_at": "2026-08-14T06:02:00Z"
        }),
    );
    std::fs::write(
        &legacy,
        serde_json::to_string_pretty(&serde_json::Value::Object(rooms)).unwrap(),
    )
    .unwrap();

    let state = Arc::new(AppState::open(&database, Some(&legacy)).await.unwrap());
    let server = start_server_with_state(state.clone()).await;
    let list: Vec<serde_json::Value> = reqwest::get(format!("{}/api/rooms", server))
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(list.len(), 2);
    assert!(list
        .iter()
        .all(|room| room["name"] != "legacy-private-without-hash"));
    assert!(PathBuf::from(format!("{}.backup", legacy.display())).exists());

    let (_sink, _stream) =
        ws_connect(&server, &private_id.to_string(), "alice", Some("secret")).await;

    server.shutdown().await;
    state.pool().close().await;
    drop(state);

    // Reopening is idempotent even while the legacy file remains in place.
    let reopened = AppState::open(&database, Some(&legacy)).await.unwrap();
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM rooms")
        .fetch_one(reopened.pool())
        .await
        .unwrap();
    assert_eq!(count, 2);
    reopened.pool().close().await;

    remove_sqlite_files(&database);
    let _ = std::fs::remove_file(&legacy);
    let _ = std::fs::remove_file(format!("{}.backup", legacy.display()));
}

#[tokio::test]
async fn corrupt_legacy_json_does_not_block_startup() {
    let database = temp_path("chat-rooms-corrupt", "db");
    let legacy = temp_path("chat-rooms-corrupt", "json");
    std::fs::write(&legacy, "{not valid json").unwrap();

    let state = AppState::open(&database, Some(&legacy)).await.unwrap();
    assert!(state.list_rooms(None).await.is_empty());
    state.pool().close().await;

    remove_sqlite_files(&database);
    let _ = std::fs::remove_file(legacy);
}

#[tokio::test]
async fn concurrent_duplicate_room_creation_returns_conflict() {
    let server = start_server().await;
    let url = format!("{}/api/rooms", server);
    let request = || {
        reqwest::Client::new()
            .post(&url)
            .json(&serde_json::json!({ "name": "same-name", "password": "" }))
            .send()
    };

    let (first, second) = tokio::join!(request(), request());
    let mut statuses = vec![
        first.unwrap().status().as_u16(),
        second.unwrap().status().as_u16(),
    ];
    statuses.sort_unstable();
    assert_eq!(statuses, vec![201, 409]);

    let rooms: Vec<serde_json::Value> = reqwest::get(&url).await.unwrap().json().await.unwrap();
    assert_eq!(rooms.len(), 1);
}

#[tokio::test]
async fn list_rooms_filter_by_name() {
    let base = start_server().await;
    create_room(&base, "alpha", None).await;
    create_room(&base, "beta", Some("pw")).await;
    create_room(&base, "gamma", None).await;

    // No filter — all three.
    let list: Vec<serde_json::Value> = reqwest::get(format!("{}/api/rooms", base))
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(list.len(), 3);

    // Filter by name "beta".
    let list: Vec<serde_json::Value> = reqwest::get(format!("{}/api/rooms?name=beta", base))
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0]["name"], "beta");

    // Filter by non-existent name.
    let list: Vec<serde_json::Value> = reqwest::get(format!("{}/api/rooms?name=nobody", base))
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(list.is_empty());

    // Filter by name with URL-encoded space (%20).
    create_room(&base, "my room", None).await;
    let list: Vec<serde_json::Value> = reqwest::get(format!("{}/api/rooms?name=my%20room", base))
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0]["name"], "my room");
}

#[tokio::test]
async fn fresh_start_creates_database_and_runs_migrations() {
    let database = temp_path("chat-rooms-fresh", "db");
    assert!(!database.exists());

    let state = Arc::new(AppState::open(&database, None).await.unwrap());
    assert!(database.exists());

    let migration_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM _sqlx_migrations")
        .fetch_one(state.pool())
        .await
        .unwrap();
    assert_eq!(migration_count, 1);

    let server = start_server_with_state(state.clone()).await;
    let list: Vec<serde_json::Value> = reqwest::get(format!("{}/api/rooms", server))
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(list.is_empty());

    server.shutdown().await;
    state.pool().close().await;
    remove_sqlite_files(&database);
}
