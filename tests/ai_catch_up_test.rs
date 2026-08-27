use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Mutex,
};

use axum::{
    body::Body, extract::State, http::header::CONTENT_TYPE, response::Response, routing::post,
    Json, Router,
};
use chat_room::{build_app, config::AppConfig, state::AppState};
use chrono::{Duration, Utc};
use reqwest::{Client, StatusCode};
use serde_json::{json, Value};
use tokio::net::TcpListener;
use uuid::Uuid;

#[derive(Default)]
struct ProviderCapture {
    calls: AtomicUsize,
    requests: Mutex<Vec<Value>>,
}

struct TestServer {
    base: String,
    state: Arc<AppState>,
    provider: Arc<ProviderCapture>,
    tasks: Vec<tokio::task::JoinHandle<()>>,
}

impl Drop for TestServer {
    fn drop(&mut self) {
        for task in &self.tasks {
            task.abort();
        }
    }
}

#[derive(Clone)]
struct Account {
    id: Uuid,
    token: String,
}

async fn provider_stream(
    State(capture): State<Arc<ProviderCapture>>,
    Json(request): Json<Value>,
) -> Response<Body> {
    capture.calls.fetch_add(1, Ordering::SeqCst);
    capture.requests.lock().unwrap().push(request);
    let body = concat!(
        "data: {\"id\":\"catch-up\",\"object\":\"chat.completion.chunk\",",
        "\"created\":1,\"model\":\"test\",\"choices\":[{\"index\":0,",
        "\"delta\":{\"content\":\"主题与决定 [S1]\"},\"finish_reason\":null}]}\n\n",
        "data: [DONE]\n\n"
    );
    Response::builder()
        .header(CONTENT_TYPE, "text/event-stream")
        .body(Body::from(body))
        .unwrap()
}

async fn start_server() -> TestServer {
    let capture = Arc::new(ProviderCapture::default());
    let provider_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let provider_address = provider_listener.local_addr().unwrap();
    let provider_task = tokio::spawn({
        let capture = capture.clone();
        async move {
            axum::serve(
                provider_listener,
                Router::new()
                    .route("/v1/chat/completions", post(provider_stream))
                    .with_state(capture),
            )
            .await
            .unwrap()
        }
    });
    let mut config = AppConfig::default();
    config.ai.enabled = true;
    config.ai.provider = "openai".into();
    config.ai.api_key_env = "PATH".into();
    config.ai.model = "catch-up-test".into();
    config.ai.base_url = Some(format!("http://{provider_address}/v1"));
    let state = Arc::new(AppState::new_with_config(&config).await.unwrap());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server_task = tokio::spawn({
        let state = state.clone();
        async move { axum::serve(listener, build_app(state)).await.unwrap() }
    });
    TestServer {
        base: format!("http://{address}"),
        state,
        provider: capture,
        tasks: vec![provider_task, server_task],
    }
}

async fn register(client: &Client, server: &TestServer, username: &str) -> Account {
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

async fn create_room(client: &Client, server: &TestServer, account: &Account) -> Uuid {
    let name = format!("Catch-up {}", Uuid::new_v4().simple());
    let room: Value = client
        .post(format!("{}/api/rooms", server.base))
        .bearer_auth(&account.token)
        .json(&json!({ "name": name, "join_policy": "open" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    Uuid::parse_str(room["id"].as_str().unwrap()).unwrap()
}

async fn create_thread(client: &Client, server: &TestServer, account: &Account) -> Uuid {
    let thread: Value = client
        .post(format!("{}/api/ai/threads", server.base))
        .bearer_auth(&account.token)
        .json(&json!({}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    Uuid::parse_str(thread["id"].as_str().unwrap()).unwrap()
}

async fn insert_message(
    server: &TestServer,
    room_id: Uuid,
    sender: &Account,
    content: &str,
    offset: i64,
    recalled: bool,
) -> Uuid {
    let id = Uuid::new_v4();
    let created_at = Utc::now() + Duration::milliseconds(offset);
    sqlx::query(
        "INSERT INTO messages \
         (id, room_id, sender_id, sender, content, recalled_at, created_at) \
         VALUES (?, ?, ?, 'source-user', ?, ?, ?)",
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

async fn wait_for_terminal(
    client: &Client,
    server: &TestServer,
    token: &str,
    run_id: Uuid,
) -> Value {
    for _ in 0..100 {
        let run: Value = client
            .get(format!("{}/api/ai/runs/{run_id}", server.base))
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
    panic!("catch-up run did not finish");
}

#[tokio::test]
async fn catch_up_uses_server_boundaries_and_persists_only_cited_sources() {
    let server = start_server().await;
    let client = Client::new();
    let reader = register(&client, &server, "catch-up-reader").await;
    let sender = register(&client, &server, "catch-up-sender").await;
    let room_id = create_room(&client, &server, &reader).await;
    let read_id = insert_message(&server, room_id, &sender, "already-read", 1, false).await;
    let cited_id = insert_message(&server, room_id, &sender, "used-marker", 2, false).await;
    insert_message(&server, room_id, &sender, "recalled-secret", 3, true).await;
    let latest_id = insert_message(&server, room_id, &sender, "unused-marker", 4, false).await;
    sqlx::query(
        "INSERT INTO room_reads (room_id, user_id, message_id, read_at) VALUES (?, ?, ?, ?)",
    )
    .bind(room_id)
    .bind(reader.id)
    .bind(read_id)
    .bind(Utc::now())
    .execute(server.state.pool())
    .await
    .unwrap();
    let thread_id = create_thread(&client, &server, &reader).await;

    let accepted = client
        .post(format!(
            "{}/api/ai/threads/{thread_id}/catch-up",
            server.base
        ))
        .bearer_auth(&reader.token)
        .json(&json!({
            "room_id": room_id,
            "client_request_id": Uuid::new_v4(),
            "source_after_message_id": Uuid::new_v4(),
            "source_through_message_id": Uuid::new_v4()
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(accepted.status(), StatusCode::ACCEPTED);
    let accepted: Value = accepted.json().await.unwrap();
    assert_eq!(accepted["purpose"], "catch_up");
    assert_eq!(accepted["source_after_message_id"], read_id.to_string());
    assert_eq!(accepted["source_through_message_id"], latest_id.to_string());
    assert_eq!(accepted["source_message_count"], 2);
    let run_id = Uuid::parse_str(accepted["id"].as_str().unwrap()).unwrap();
    let completed = wait_for_terminal(&client, &server, &reader.token, run_id).await;
    assert_eq!(completed["status"], "completed");
    assert_eq!(completed["context_message_count"], 2);

    let request = server.provider.requests.lock().unwrap()[0].to_string();
    assert!(request.contains("used-marker"));
    assert!(request.contains("unused-marker"));
    assert!(!request.contains("already-read"));
    assert!(!request.contains("recalled-secret"));
    let messages: Vec<Value> = client
        .get(format!(
            "{}/api/ai/threads/{thread_id}/messages",
            server.base
        ))
        .bearer_auth(&reader.token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let answer = messages
        .iter()
        .find(|message| message["role"] == "assistant")
        .unwrap();
    assert_eq!(answer["sources"].as_array().unwrap().len(), 1);
    assert_eq!(answer["sources"][0]["message_id"], cited_id.to_string());
    let hidden_room = create_room(&client, &server, &sender).await;
    let denied = client
        .post(format!(
            "{}/api/ai/threads/{thread_id}/catch-up",
            server.base
        ))
        .bearer_auth(&reader.token)
        .json(&json!({ "room_id": hidden_room, "client_request_id": Uuid::new_v4() }))
        .send()
        .await
        .unwrap();
    assert_eq!(denied.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn catch_up_skips_the_model_when_there_are_no_incoming_unreads() {
    let server = start_server().await;
    let client = Client::new();
    let reader = register(&client, &server, "caught-up-reader").await;
    let room_id = create_room(&client, &server, &reader).await;
    insert_message(&server, room_id, &reader, "my-own-message", 1, false).await;
    let thread_id = create_thread(&client, &server, &reader).await;
    let response = client
        .post(format!(
            "{}/api/ai/threads/{thread_id}/catch-up",
            server.base
        ))
        .bearer_auth(&reader.token)
        .json(&json!({ "room_id": room_id, "client_request_id": Uuid::new_v4() }))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert_eq!(server.provider.calls.load(Ordering::SeqCst), 0);
    let run_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM ai_runs")
        .fetch_one(server.state.pool())
        .await
        .unwrap();
    assert_eq!(run_count, 0);
}

#[tokio::test]
async fn catch_up_context_is_bounded_to_the_latest_five_hundred_messages() {
    let server = start_server().await;
    let client = Client::new();
    let reader = register(&client, &server, "bounded-reader").await;
    let sender = register(&client, &server, "bounded-sender").await;
    let room_id = create_room(&client, &server, &reader).await;
    for index in 0..505 {
        insert_message(
            &server,
            room_id,
            &sender,
            &format!("bounded-marker-{index:03}"),
            index,
            false,
        )
        .await;
    }
    let thread_id = create_thread(&client, &server, &reader).await;
    let accepted: Value = client
        .post(format!(
            "{}/api/ai/threads/{thread_id}/catch-up",
            server.base
        ))
        .bearer_auth(&reader.token)
        .json(&json!({ "room_id": room_id, "client_request_id": Uuid::new_v4() }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(accepted["source_message_count"], 505);
    let run_id = Uuid::parse_str(accepted["id"].as_str().unwrap()).unwrap();
    let completed = wait_for_terminal(&client, &server, &reader.token, run_id).await;
    assert_eq!(completed["context_message_count"], 500);
    let request = server.provider.requests.lock().unwrap()[0].to_string();
    assert!(!request.contains("bounded-marker-000"));
    assert!(request.contains("bounded-marker-504"));
}
