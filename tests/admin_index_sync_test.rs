use std::sync::Arc;

use chat_room::{
    build_app,
    config::{AdminConfig, AppConfig, VectorStoreConfig},
    state::AppState,
};
use chrono::Utc;
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use uuid::Uuid;

mod support;
use support::{session_token, system_admin_token};

type Socket =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

struct Server {
    base: String,
    state: Arc<AppState>,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn start() -> Server {
    let config = AppConfig {
        admin: AdminConfig {
            usernames: vec!["index-admin".into()],
            ..AdminConfig::default()
        },
        vector_store: VectorStoreConfig {
            enabled: true,
            url: "http://127.0.0.1:9".into(),
            collection: "test-messages".into(),
            dimensions: 2,
            embedding_base_url: "http://127.0.0.1:9/v1".into(),
            embedding_model: "embed-test".into(),
            worker_interval_ms: 60_000,
            ..VectorStoreConfig::default()
        },
        ..AppConfig::default()
    };
    let state = Arc::new(AppState::new_with_config(&config).await.unwrap());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn({
        let state = state.clone();
        async move { axum::serve(listener, build_app(state)).await.unwrap() }
    });
    Server { base, state, task }
}

async fn create_room(base: &str, token: &str) -> Uuid {
    let room: serde_json::Value = reqwest::Client::new()
        .post(format!("{base}/api/rooms"))
        .bearer_auth(token)
        .json(&serde_json::json!({ "name": "index sync room", "password": "" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    Uuid::parse_str(room["id"].as_str().unwrap()).unwrap()
}

async fn next_type(socket: &mut Socket, expected: &str) -> serde_json::Value {
    loop {
        let frame = tokio::time::timeout(std::time::Duration::from_secs(3), socket.next())
            .await
            .expect("timed out waiting for WebSocket frame")
            .expect("WebSocket ended")
            .expect("WebSocket error");
        let Message::Text(text) = frame else { continue };
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        if value["type"] == expected {
            return value;
        }
    }
}

async fn open_room(base: &str, room_id: Uuid, token: &str) -> Socket {
    let url = format!("{}/ws/{room_id}", base.replacen("http://", "ws://", 1));
    let (mut socket, _) = connect_async(url).await.unwrap();
    socket
        .send(Message::Text(
            serde_json::json!({ "type": "join", "token": token }).to_string(),
        ))
        .await
        .unwrap();
    let _ = next_type(&mut socket, "auth_ok").await;
    socket
}

#[tokio::test]
async fn message_changes_are_automatically_queued_for_vector_sync() {
    let server = start().await;
    let token = session_token(&server.base, "automatic-index-owner").await;
    let room_id = create_room(&server.base, &token).await;
    let mut socket = open_room(&server.base, room_id, &token).await;
    socket
        .send(Message::Text(
            serde_json::json!({
                "type": "message",
                "content": "first indexed version",
                "client_message_id": Uuid::new_v4(),
            })
            .to_string(),
        ))
        .await
        .unwrap();
    let sent = next_type(&mut socket, "broadcast").await;
    let message_id = Uuid::parse_str(sent["message_id"].as_str().unwrap()).unwrap();

    let inserted: (String, i64) = sqlx::query_as(
        "SELECT operation, generation FROM message_index_outbox WHERE message_id = ?",
    )
    .bind(message_id)
    .fetch_one(server.state.pool())
    .await
    .unwrap();
    assert_eq!(inserted, ("upsert".into(), 1));

    socket
        .send(Message::Text(
            serde_json::json!({
                "type": "edit",
                "message_id": message_id,
                "content": "updated indexed version",
            })
            .to_string(),
        ))
        .await
        .unwrap();
    let _ = next_type(&mut socket, "message_edited").await;
    let edited: (String, i64) = sqlx::query_as(
        "SELECT operation, generation FROM message_index_outbox WHERE message_id = ?",
    )
    .bind(message_id)
    .fetch_one(server.state.pool())
    .await
    .unwrap();
    assert_eq!(edited, ("upsert".into(), 2));

    socket
        .send(Message::Text(
            serde_json::json!({ "type": "recall", "message_id": message_id }).to_string(),
        ))
        .await
        .unwrap();
    let _ = next_type(&mut socket, "message_recalled").await;
    let recalled: (String, i64) = sqlx::query_as(
        "SELECT operation, generation FROM message_index_outbox WHERE message_id = ?",
    )
    .bind(message_id)
    .fetch_one(server.state.pool())
    .await
    .unwrap();
    assert_eq!(recalled, ("delete".into(), 3));
}

#[tokio::test]
async fn admin_can_resync_the_vector_outbox() {
    let server = start().await;
    let graph_tables: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'message_graph_outbox'",
    )
    .fetch_one(server.state.pool())
    .await
    .unwrap();
    assert_eq!(graph_tables, 0);
    let client = reqwest::Client::new();
    let admin_token = system_admin_token(&server.state, &server.base, "index-admin").await;
    let regular_token = session_token(&server.base, "index-user").await;
    let admin = server
        .state
        .session_user(Uuid::parse_str(&admin_token).unwrap())
        .await
        .unwrap()
        .unwrap();
    let room_id = create_room(&server.base, &admin_token).await;
    let active_id = Uuid::new_v4();
    let recalled_id = Uuid::new_v4();
    for (id, content, recalled_at) in [
        (active_id, "current text", None),
        (recalled_id, "recalled text", Some(Utc::now())),
    ] {
        sqlx::query(
            "INSERT INTO messages (id, room_id, sender_id, sender, content, recalled_at, created_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(id)
        .bind(room_id)
        .bind(admin.id)
        .bind(&admin.username)
        .bind(content)
        .bind(recalled_at)
        .bind(Utc::now())
        .execute(server.state.pool())
        .await
        .unwrap();
    }
    sqlx::query("DELETE FROM message_index_outbox")
        .execute(server.state.pool())
        .await
        .unwrap();

    let unauthorized = client
        .post(format!("{}/api/admin/indexes/sync", server.base))
        .bearer_auth(&regular_token)
        .json(&serde_json::json!({ "target": "vector" }))
        .send()
        .await
        .unwrap();
    assert_eq!(unauthorized.status(), 403);

    let response = client
        .post(format!("{}/api/admin/indexes/sync", server.base))
        .bearer_auth(&admin_token)
        .json(&serde_json::json!({ "target": "vector" }))
        .send()
        .await
        .unwrap();
    let status = response.status();
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(status, 200, "sync response: {body}");
    assert_eq!(body["target"], "vector");
    assert_eq!(body["queued_messages"], 2);

    let vector_jobs: Vec<(Uuid, String, i64, Option<String>)> = sqlx::query_as(
        "SELECT message_id, operation, attempt_count, last_error \
         FROM message_index_outbox ORDER BY message_id",
    )
    .fetch_all(server.state.pool())
    .await
    .unwrap();
    assert_eq!(vector_jobs.len(), 2);
    assert!(vector_jobs.contains(&(active_id, "upsert".into(), 0, None)));
    assert!(vector_jobs.contains(&(recalled_id, "delete".into(), 0, None)));
}
