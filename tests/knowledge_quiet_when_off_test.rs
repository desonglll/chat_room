//! TG-1207: the knowledge indexer must be silent when AI is off, and must stop (not retry
//! forever) when the embedding provider rejects its credentials.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::{
    extract::State,
    http::{StatusCode, Uri},
    response::IntoResponse,
    routing::any,
    Json, Router,
};
use chat_room::{
    ai::AiConfig,
    build_app,
    config::{AppConfig, VectorStoreConfig},
    state::AppState,
};
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use uuid::Uuid;

mod support;
use support::session_token;

type Calls = Arc<Mutex<Vec<String>>>;
type Socket =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

/// Fake Qdrant + embeddings provider: Qdrant calls succeed, `/embeddings` answers 401.
async fn fake_provider() -> (String, Calls) {
    async fn handle(State(calls): State<Calls>, uri: Uri) -> impl IntoResponse {
        calls.lock().unwrap().push(uri.path().to_string());
        if uri.path() == "/embeddings" {
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({ "error": "invalid api key" })),
            )
                .into_response();
        }
        Json(serde_json::json!({ "result": {} })).into_response()
    }
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let calls: Calls = Arc::new(Mutex::new(Vec::new()));
    let state = calls.clone();
    tokio::spawn(async move {
        axum::serve(
            listener,
            Router::new().fallback(any(handle)).with_state(state),
        )
        .await
        .unwrap();
    });
    (base, calls)
}

struct Server {
    base: String,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn start(ai_enabled: bool, provider: &str) -> Server {
    let config = AppConfig {
        ai: AiConfig {
            enabled: ai_enabled,
            ..AiConfig::default()
        },
        vector_store: VectorStoreConfig {
            enabled: true,
            url: provider.into(),
            collection: "quiet-test".into(),
            dimensions: 2,
            embedding_base_url: provider.into(),
            embedding_model: "embed-test".into(),
            worker_interval_ms: 50,
            ..VectorStoreConfig::default()
        },
        ..AppConfig::default()
    };
    let state = Arc::new(AppState::new_with_config(&config).await.unwrap());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move { axum::serve(listener, build_app(state)).await.unwrap() });
    Server { base, task }
}

async fn chat_socket(base: &str, owner: &str) -> Socket {
    let token = session_token(base, owner).await;
    let chat: serde_json::Value = reqwest::Client::new()
        .post(format!("{base}/api/chats"))
        .bearer_auth(&token)
        .json(&serde_json::json!({ "name": "quiet index chat", "password": "" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let url = format!(
        "{}/ws/{}",
        base.replacen("http://", "ws://", 1),
        chat["id"].as_str().unwrap()
    );
    let (mut socket, _) = connect_async(url).await.unwrap();
    socket
        .send(Message::Text(
            serde_json::json!({ "type": "join", "token": token }).to_string(),
        ))
        .await
        .unwrap();
    wait_for(&mut socket, "auth_ok").await;
    socket
}

async fn wait_for(socket: &mut Socket, expected: &str) {
    loop {
        let frame = tokio::time::timeout(Duration::from_secs(3), socket.next())
            .await
            .expect("timed out waiting for WebSocket frame")
            .expect("WebSocket ended")
            .expect("WebSocket error");
        let Message::Text(text) = frame else { continue };
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        if value["type"] == expected {
            return;
        }
    }
}

async fn send_messages(socket: &mut Socket, count: usize) {
    for index in 0..count {
        socket
            .send(Message::Text(
                serde_json::json!({
                    "type": "message",
                    "content": format!("index me {index}"),
                    "client_message_id": Uuid::new_v4(),
                })
                .to_string(),
            ))
            .await
            .unwrap();
        wait_for(socket, "broadcast").await;
    }
}

fn embedding_calls(calls: &Calls) -> usize {
    calls
        .lock()
        .unwrap()
        .iter()
        .filter(|path| path.as_str() == "/embeddings")
        .count()
}

#[tokio::test]
async fn ai_off_means_no_vector_or_embedding_traffic_even_with_the_vector_flag_on() {
    let (provider, calls) = fake_provider().await;
    let server = start(false, &provider).await;
    let mut socket = chat_socket(&server.base, "quiet-off-owner").await;
    send_messages(&mut socket, 3).await;
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert!(
        calls.lock().unwrap().is_empty(),
        "AI is off, yet the indexer called {:?}",
        calls.lock().unwrap()
    );
}

#[tokio::test]
async fn rejected_embedding_credentials_stop_indexing_instead_of_retrying_forever() {
    let (provider, calls) = fake_provider().await;
    let server = start(true, &provider).await;
    let mut socket = chat_socket(&server.base, "quiet-401-owner").await;
    send_messages(&mut socket, 3).await;
    // The first batch hits the 401 (at most one concurrent batch of jobs).
    let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
    while embedding_calls(&calls) == 0 {
        assert!(tokio::time::Instant::now() < deadline, "indexer never ran");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    tokio::time::sleep(Duration::from_millis(300)).await;
    let after_rejection = embedding_calls(&calls);
    assert!(
        after_rejection <= 4,
        "one batch at most, saw {after_rejection}"
    );

    // New messages and the retry backoff (1 s, 2 s) must not reach the provider again.
    send_messages(&mut socket, 3).await;
    tokio::time::sleep(Duration::from_millis(2_500)).await;
    assert_eq!(
        embedding_calls(&calls),
        after_rejection,
        "indexing kept calling a provider that rejected the credentials"
    );
}
