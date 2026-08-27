use std::sync::{Arc, Mutex};

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
struct ProviderCapture(Mutex<Vec<Value>>);

struct TestServer {
    base: String,
    state: Arc<AppState>,
    capture: Arc<ProviderCapture>,
    tasks: Vec<tokio::task::JoinHandle<()>>,
}

impl Drop for TestServer {
    fn drop(&mut self) {
        for task in &self.tasks {
            task.abort();
        }
    }
}

async fn provider(
    State(capture): State<Arc<ProviderCapture>>,
    Json(request): Json<Value>,
) -> Response<Body> {
    let planning = request.to_string().contains("planning agent")
        || request.to_string().contains("context_scope");
    capture.0.lock().unwrap().push(request);
    if planning {
        let content = json!({
            "intent": "general",
            "context_scope": "full",
            "semantic_search": true,
            "research_questions": ["must-not-expand"]
        })
        .to_string();
        return Response::builder()
            .header(CONTENT_TYPE, "application/json")
            .body(Body::from(
                json!({
                    "id": "plan",
                    "object": "chat.completion",
                    "created": 1,
                    "model": "test",
                    "choices": [{ "index": 0, "message": { "role": "assistant", "content": content }, "finish_reason": "stop" }]
                })
                .to_string(),
            ))
            .unwrap();
    }
    let body = concat!(
        "data: {\"id\":\"answer\",\"object\":\"chat.completion.chunk\",\"created\":1,",
        "\"model\":\"test\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"selected answer\"},",
        "\"finish_reason\":null}]}\n\ndata: [DONE]\n\n"
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
                    .route("/v1/chat/completions", post(provider))
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
    config.ai.model = "selected-context-test".into();
    config.ai.base_url = Some(format!("http://{provider_address}/v1"));
    let state = Arc::new(AppState::new_with_config(&config).await.unwrap());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let server_task = tokio::spawn({
        let state = state.clone();
        async move { axum::serve(listener, build_app(state)).await.unwrap() }
    });
    TestServer {
        base,
        state,
        capture,
        tasks: vec![provider_task, server_task],
    }
}

async fn register(client: &Client, server: &TestServer) -> (Uuid, String) {
    let session: Value = client
        .post(format!("{}/api/users/register", server.base))
        .json(&json!({ "username": "selected-context-owner", "password": "test-password" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    (
        Uuid::parse_str(session["user"]["id"].as_str().unwrap()).unwrap(),
        session["token"].as_str().unwrap().into(),
    )
}

async fn create_room(client: &Client, server: &TestServer, token: &str, name: &str) -> Uuid {
    let room: Value = client
        .post(format!("{}/api/rooms", server.base))
        .bearer_auth(token)
        .json(&json!({ "name": name, "join_policy": "open" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    Uuid::parse_str(room["id"].as_str().unwrap()).unwrap()
}

async fn create_thread(
    client: &Client,
    server: &TestServer,
    token: &str,
    room_id: Option<Uuid>,
) -> Uuid {
    let thread: Value = client
        .post(format!("{}/api/ai/threads", server.base))
        .bearer_auth(token)
        .json(&json!({ "room_id": room_id }))
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
    sender_id: Uuid,
    content: &str,
    offset: i64,
    recalled: bool,
) -> Uuid {
    let id = Uuid::new_v4();
    let created_at = Utc::now() + Duration::milliseconds(offset);
    sqlx::query(
        "INSERT INTO messages (id, room_id, sender_id, sender, content, recalled_at, created_at) \
         VALUES (?, ?, ?, 'owner', ?, ?, ?)",
    )
    .bind(id)
    .bind(room_id)
    .bind(sender_id)
    .bind(content)
    .bind(recalled.then_some(created_at))
    .bind(created_at)
    .execute(server.state.pool())
    .await
    .unwrap();
    id
}

async fn run_request(
    client: &Client,
    server: &TestServer,
    token: &str,
    thread_id: Uuid,
    message_ids: Vec<Uuid>,
) -> reqwest::Response {
    client
        .post(format!("{}/api/ai/threads/{thread_id}/runs", server.base))
        .bearer_auth(token)
        .json(&json!({
            "question": "What do these messages mean?",
            "client_request_id": Uuid::new_v4(),
            "message_ids": message_ids
        }))
        .send()
        .await
        .unwrap()
}

#[tokio::test]
async fn selected_message_runs_are_exact_ordered_and_validated() {
    let server = start_server().await;
    let client = Client::new();
    let (user_id, token) = register(&client, &server).await;
    let room_id = create_room(&client, &server, &token, "Selected context").await;
    let other_room_id = create_room(&client, &server, &token, "Other room").await;
    let first = insert_message(&server, room_id, user_id, "selected-first", 1, false).await;
    let ignored = insert_message(&server, room_id, user_id, "must-not-be-used", 2, false).await;
    let last = insert_message(&server, room_id, user_id, "selected-last", 3, false).await;
    let recalled = insert_message(&server, room_id, user_id, "recalled-secret", 4, true).await;
    let other = insert_message(
        &server,
        other_room_id,
        user_id,
        "other-room-secret",
        5,
        false,
    )
    .await;
    let thread_id = create_thread(&client, &server, &token, Some(room_id)).await;

    let accepted = run_request(&client, &server, &token, thread_id, vec![last, first]).await;
    assert_eq!(accepted.status(), StatusCode::ACCEPTED);
    let run: Value = accepted.json().await.unwrap();
    let run_id = Uuid::parse_str(run["id"].as_str().unwrap()).unwrap();
    for _ in 0..100 {
        let status: String = sqlx::query_scalar("SELECT status FROM ai_runs WHERE id = ?")
            .bind(run_id)
            .fetch_one(server.state.pool())
            .await
            .unwrap();
        if matches!(status.as_str(), "completed" | "failed") {
            assert_eq!(status, "completed");
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    let selected: Vec<Uuid> = sqlx::query_scalar(
        "SELECT message_id FROM ai_run_selected_messages WHERE run_id = ? ORDER BY ordinal",
    )
    .bind(run_id)
    .fetch_all(server.state.pool())
    .await
    .unwrap();
    assert_eq!(selected, [last, first]);
    let generated = server.capture.0.lock().unwrap().last().unwrap().to_string();
    assert!(generated.contains("selected-last"));
    assert!(generated.contains("selected-first"));
    assert!(!generated.contains("must-not-be-used"));
    assert!(!generated.contains("must-not-expand"));
    assert!(generated.find("selected-last") < generated.find("selected-first"));
    let context_count: i64 =
        sqlx::query_scalar("SELECT context_message_count FROM ai_runs WHERE id = ?")
            .bind(run_id)
            .fetch_one(server.state.pool())
            .await
            .unwrap();
    assert_eq!(context_count, 2);

    assert_eq!(
        run_request(&client, &server, &token, thread_id, vec![other])
            .await
            .status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        run_request(&client, &server, &token, thread_id, vec![recalled])
            .await
            .status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        run_request(&client, &server, &token, thread_id, vec![first, first])
            .await
            .status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        run_request(&client, &server, &token, thread_id, vec![ignored; 51])
            .await
            .status(),
        StatusCode::BAD_REQUEST
    );
    let personal = create_thread(&client, &server, &token, None).await;
    assert_eq!(
        run_request(&client, &server, &token, personal, vec![first])
            .await
            .status(),
        StatusCode::BAD_REQUEST
    );
}
