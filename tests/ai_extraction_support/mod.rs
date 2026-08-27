use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Mutex,
};

use axum::{extract::State, routing::post, Json, Router};
use chat_room::{build_app, config::AppConfig, state::AppState};
use chrono::Utc;
use reqwest::Client;
use serde_json::{json, Value};
use tokio::net::TcpListener;
use uuid::Uuid;

#[derive(Default)]
pub struct ProviderState {
    pub calls: AtomicUsize,
    pub requests: Mutex<Vec<Value>>,
}

pub struct TestServer {
    pub base: String,
    pub state: Arc<AppState>,
    pub provider: Arc<ProviderState>,
    tasks: Vec<tokio::task::JoinHandle<()>>,
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.tasks.iter().for_each(tokio::task::JoinHandle::abort);
    }
}

#[derive(Clone)]
pub struct Account {
    pub id: Uuid,
    pub token: String,
}

async fn provider_chat(
    State(state): State<Arc<ProviderState>>,
    Json(request): Json<Value>,
) -> Json<Value> {
    state.calls.fetch_add(1, Ordering::SeqCst);
    let invalid_source = request.to_string().contains("invalid-source-marker");
    state.requests.lock().unwrap().push(request);
    let content = if invalid_source {
        json!({
            "candidates": [{
                "kind": "task",
                "title": "Invalid source",
                "detail": "Must not persist",
                "source_labels": ["S999"]
            }]
        })
    } else {
        json!({
        "candidates": [
            { "kind": "decision", "title": "Friday release", "detail": "The room approved Friday", "source_labels": ["S1"] },
            { "kind": "task", "title": "Prepare release notes", "detail": "Notes are needed before release", "source_labels": ["S2"] },
            { "kind": "task", "title": "Prepare   release notes", "detail": "Duplicate wording", "source_labels": ["S2", "S2"] },
            { "kind": "task", "title": "Arrange a retrospective", "detail": "This was not stated directly", "source_labels": [] }
        ]
        })
    }
    .to_string();
    Json(json!({
        "id": "chatcmpl-extraction",
        "object": "chat.completion",
        "created": 1,
        "model": "extraction-test",
        "choices": [{ "index": 0, "message": { "role": "assistant", "content": content }, "finish_reason": "stop" }]
    }))
}

pub async fn start_server() -> TestServer {
    let provider = Arc::new(ProviderState::default());
    let provider_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let provider_address = provider_listener.local_addr().unwrap();
    let provider_task = tokio::spawn({
        let provider = provider.clone();
        async move {
            axum::serve(
                provider_listener,
                Router::new()
                    .route("/v1/chat/completions", post(provider_chat))
                    .with_state(provider),
            )
            .await
            .unwrap()
        }
    });
    let mut config = AppConfig::default();
    config.ai.enabled = true;
    config.ai.provider = "openai".into();
    config.ai.api_key_env = "PATH".into();
    config.ai.model = "extraction-test".into();
    config.ai.base_url = Some(format!("http://{provider_address}/v1"));
    let state = Arc::new(AppState::new_with_config(&config).await.unwrap());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let app_task = tokio::spawn({
        let state = state.clone();
        async move { axum::serve(listener, build_app(state)).await.unwrap() }
    });
    TestServer {
        base: format!("http://{address}"),
        state,
        provider,
        tasks: vec![provider_task, app_task],
    }
}

pub async fn register(client: &Client, server: &TestServer, username: &str) -> Account {
    let session: Value = client
        .post(format!("{}/api/users/register", server.base))
        .json(&json!({ "username": username, "password": "test-password" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    Account {
        id: Uuid::parse_str(session["user"]["id"].as_str().unwrap()).unwrap(),
        token: session["token"].as_str().unwrap().into(),
    }
}

pub async fn create_room(client: &Client, server: &TestServer, owner: &Account) -> Uuid {
    let room: Value = client
        .post(format!("{}/api/rooms", server.base))
        .bearer_auth(&owner.token)
        .json(&json!({
            "name": format!("Extraction {}", Uuid::new_v4().simple()),
            "join_policy": "open"
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    Uuid::parse_str(room["id"].as_str().unwrap()).unwrap()
}

pub async fn insert_message(
    server: &TestServer,
    room_id: Uuid,
    sender: &Account,
    content: &str,
    created_at: chrono::DateTime<Utc>,
    recalled: bool,
) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO messages (id, room_id, sender_id, sender, content, recalled_at, created_at) \
         VALUES (?, ?, ?, 'extract-owner', ?, ?, ?)",
    )
    .bind(id)
    .bind(room_id)
    .bind(sender.id)
    .bind(content)
    .bind(recalled.then_some(created_at))
    .bind(created_at)
    .execute(server.state.pool())
    .await
    .unwrap();
    id
}

pub async fn create_extraction(
    client: &Client,
    server: &TestServer,
    owner: &Account,
    room_id: Uuid,
    from_at: chrono::DateTime<Utc>,
    to_at: chrono::DateTime<Utc>,
) -> Value {
    let response = client
        .post(format!(
            "{}/api/rooms/{room_id}/ai/extractions",
            server.base
        ))
        .bearer_auth(&owner.token)
        .json(&json!({ "from_at": from_at, "to_at": to_at, "client_request_id": Uuid::new_v4() }))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::ACCEPTED);
    response.json().await.unwrap()
}

pub async fn wait_for_terminal(
    client: &Client,
    server: &TestServer,
    token: &str,
    run_id: &str,
) -> Value {
    for _ in 0..100 {
        let run: Value = client
            .get(format!("{}/api/ai/extractions/{run_id}", server.base))
            .bearer_auth(token)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        if matches!(run["status"].as_str(), Some("completed" | "failed")) {
            return run;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    panic!("AI extraction run did not finish");
}

pub async fn mutate_candidate(
    client: &Client,
    server: &TestServer,
    owner: &Account,
    candidate: &Value,
    action: &str,
) -> reqwest::Response {
    client
        .patch(format!(
            "{}/api/ai/extraction-candidates/{}",
            server.base,
            candidate["id"].as_str().unwrap()
        ))
        .bearer_auth(&owner.token)
        .json(&json!({ "action": action, "version": candidate["version"] }))
        .send()
        .await
        .unwrap()
}
