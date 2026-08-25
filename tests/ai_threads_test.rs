use std::sync::Arc;

use std::convert::Infallible;

use axum::{
    body::{Body, Bytes},
    http::header::CONTENT_TYPE,
    response::Response,
    routing::post,
    Router,
};
use chat_room::{ai::SaveAiModelOption, config::AppConfig};
use chat_room::{build_app, state::AppState};
use reqwest::{Client, StatusCode};
use tokio::net::TcpListener;

async fn start_server() -> (String, Arc<AppState>, tokio::task::JoinHandle<()>) {
    let state = Arc::new(AppState::new().await.unwrap());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server_state = state.clone();
    let task = tokio::spawn(async move {
        axum::serve(listener, build_app(server_state))
            .await
            .unwrap()
    });
    (format!("http://{address}"), state, task)
}

async fn start_server_with_config(
    config: &AppConfig,
) -> (String, Arc<AppState>, tokio::task::JoinHandle<()>) {
    let state = Arc::new(AppState::new_with_config(config).await.unwrap());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server_state = state.clone();
    let task = tokio::spawn(async move {
        axum::serve(listener, build_app(server_state))
            .await
            .unwrap()
    });
    (format!("http://{address}"), state, task)
}

async fn register(client: &Client, base: &str, username: &str) -> String {
    client
        .post(format!("{base}/api/users/register"))
        .json(&serde_json::json!({ "username": username, "password": "test-password" }))
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap()["token"]
        .as_str()
        .unwrap()
        .to_owned()
}

#[tokio::test]
async fn ai_threads_are_persistent_editable_and_private() {
    let (base, state, task) = start_server().await;
    let client = Client::new();
    let owner = register(&client, &base, "thread-owner").await;
    let outsider = register(&client, &base, "thread-outsider").await;

    let created = client
        .post(format!("{base}/api/ai/threads"))
        .bearer_auth(&owner)
        .json(&serde_json::json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(created.status(), StatusCode::CREATED);
    let created = created.json::<serde_json::Value>().await.unwrap();
    assert_eq!(created["title"], "新对话");
    assert_eq!(created["thinking_enabled"], false);
    assert!(created["room_id"].is_null());
    let thread_id = created["id"].as_str().unwrap();
    let thread_id = uuid::Uuid::parse_str(thread_id).unwrap();

    let updated = client
        .patch(format!("{base}/api/ai/threads/{thread_id}"))
        .bearer_auth(&owner)
        .json(&serde_json::json!({ "title": "项目回顾", "thinking_enabled": true }))
        .send()
        .await
        .unwrap();
    assert_eq!(updated.status(), StatusCode::OK);
    let updated = updated.json::<serde_json::Value>().await.unwrap();
    assert_eq!(updated["title"], "项目回顾");
    assert_eq!(updated["thinking_enabled"], true);

    let threads = client
        .get(format!("{base}/api/ai/threads"))
        .bearer_auth(&owner)
        .send()
        .await
        .unwrap()
        .json::<Vec<serde_json::Value>>()
        .await
        .unwrap();
    assert_eq!(threads.len(), 1);
    assert_eq!(threads[0]["id"], thread_id.to_string());

    let owner_token = uuid::Uuid::parse_str(&owner).unwrap();
    let owner_user = state.session_user(owner_token).await.unwrap().unwrap();
    state
        .append_ai_thread_message(owner_user.id, thread_id, "user", "第一问", None, None)
        .await
        .unwrap()
        .unwrap();
    state
        .append_ai_thread_message(
            owner_user.id,
            thread_id,
            "assistant",
            "第一答",
            None,
            Some(0),
        )
        .await
        .unwrap()
        .unwrap();
    let messages = client
        .get(format!("{base}/api/ai/threads/{thread_id}/messages"))
        .bearer_auth(&owner)
        .send()
        .await
        .unwrap()
        .json::<Vec<serde_json::Value>>()
        .await
        .unwrap();
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0]["content"], "第一问");
    assert_eq!(messages[1]["content"], "第一答");

    assert_eq!(
        client
            .get(format!("{base}/api/ai/threads/{thread_id}/messages"))
            .bearer_auth(&outsider)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::NOT_FOUND
    );
    task.abort();
}

#[tokio::test]
async fn ai_run_continues_without_a_browser_stream() {
    async fn openai_stream() -> Response<Body> {
        let chunks = [
            "data: {\"id\":\"chatcmpl-test\",\"object\":\"chat.completion.chunk\",\"created\":1,\"model\":\"gpt-test\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"content\":\"你\"},\"finish_reason\":null}]}\n\n",
            "data: {\"id\":\"chatcmpl-test\",\"object\":\"chat.completion.chunk\",\"created\":1,\"model\":\"gpt-test\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"好\"},\"finish_reason\":null}]}\n\n",
            concat!(
                "data: {\"id\":\"chatcmpl-test\",\"object\":\"chat.completion.chunk\",\"created\":1,\"model\":\"gpt-test\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n",
                "data: [DONE]\n\n"
            ),
        ];
        let stream = futures_util::stream::unfold(0, move |index| async move {
            if index >= chunks.len() {
                return None;
            }
            if index == 1 {
                tokio::time::sleep(std::time::Duration::from_millis(75)).await;
            } else if index == 2 {
                tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            }
            Some((
                Ok::<_, Infallible>(Bytes::from_static(chunks[index].as_bytes())),
                index + 1,
            ))
        });
        Response::builder()
            .header(CONTENT_TYPE, "text/event-stream")
            .body(Body::from_stream(stream))
            .unwrap()
    }

    let provider_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let provider_address = provider_listener.local_addr().unwrap();
    let provider = tokio::spawn(async move {
        axum::serve(
            provider_listener,
            Router::new().route("/v1/chat/completions", post(openai_stream)),
        )
        .await
        .unwrap()
    });
    let mut config = AppConfig::default();
    config.ai.enabled = true;
    config.ai.provider = "openai".into();
    config.ai.api_key_env = "PATH".into();
    config.ai.model = "gpt-test".into();
    config.ai.base_url = Some(format!("http://{provider_address}/v1"));
    config.redis.enabled = true;
    let (base, state, server) = start_server_with_config(&config).await;
    let client = Client::new();
    let token = register(&client, &base, "durable-run-owner").await;
    let selected_model = state
        .create_ai_model_option(&SaveAiModelOption {
            label: "Selected provider".into(),
            provider: "openai".into(),
            base_url: format!("http://{provider_address}/v1"),
            model: "gpt-selected".into(),
            api_key_env: "PATH".into(),
            enabled: true,
        })
        .await
        .unwrap();
    let thread: serde_json::Value = client
        .post(format!("{base}/api/ai/threads"))
        .bearer_auth(&token)
        .json(&serde_json::json!({}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let thread_id = thread["id"].as_str().unwrap();

    let accepted = client
        .post(format!("{base}/api/ai/threads/{thread_id}/runs"))
        .bearer_auth(&token)
        .json(&serde_json::json!({
            "question": "你好",
            "client_request_id": uuid::Uuid::new_v4(),
            "model_option_id": selected_model.id
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(accepted.status(), StatusCode::ACCEPTED);
    let accepted: serde_json::Value = accepted.json().await.unwrap();
    assert_eq!(accepted["model"], "gpt-selected");
    let run_id = accepted["id"].as_str().unwrap();
    let events = client
        .get(format!("{base}/api/ai/runs/{run_id}/events"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(events.status(), StatusCode::OK);
    let events_task = tokio::spawn(async move { events.text().await.unwrap() });

    let mut live_revision = None;
    for _ in 0..20 {
        let messages = client
            .get(format!("{base}/api/ai/threads/{thread_id}/messages"))
            .bearer_auth(&token)
            .send()
            .await
            .unwrap()
            .json::<Vec<serde_json::Value>>()
            .await
            .unwrap();
        live_revision = messages
            .into_iter()
            .find(|message| message["role"] == "assistant" && message["status"] == "streaming");
        if live_revision.is_some() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    assert_eq!(live_revision.unwrap()["content"], "你好");
    let persisted_during_stream: (String, String) = sqlx::query_as(
        "SELECT content, status FROM ai_thread_messages WHERE thread_id = $1 AND role = 'assistant'",
    )
    .bind(uuid::Uuid::parse_str(thread_id).unwrap())
    .fetch_one(state.pool())
    .await
    .unwrap();
    assert_eq!(persisted_during_stream, (String::new(), "pending".into()));

    let mut completed = None;
    for _ in 0..40 {
        let messages = client
            .get(format!("{base}/api/ai/threads/{thread_id}/messages"))
            .bearer_auth(&token)
            .send()
            .await
            .unwrap()
            .json::<Vec<serde_json::Value>>()
            .await
            .unwrap();
        completed = messages
            .into_iter()
            .find(|message| message["role"] == "assistant" && message["status"] == "completed");
        if completed.is_some() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }

    let completed = completed.unwrap();
    assert_eq!(completed["content"], "你好");
    assert_eq!(completed["retrieved_message_count"], 0);
    let event_body = tokio::time::timeout(std::time::Duration::from_secs(3), events_task)
        .await
        .unwrap()
        .unwrap();
    assert!(event_body.contains("\"status\":\"streaming\""));
    assert!(event_body.contains("\"status\":\"completed\""));
    let persisted_after_completion: (String, String, i64) = sqlx::query_as(
        "SELECT content, status, retrieved_message_count FROM ai_thread_messages \
         WHERE thread_id = $1 AND role = 'assistant'",
    )
    .bind(uuid::Uuid::parse_str(thread_id).unwrap())
    .fetch_one(state.pool())
    .await
    .unwrap();
    assert_eq!(
        persisted_after_completion,
        ("你好".into(), "completed".into(), 0)
    );
    server.abort();
    provider.abort();
}
