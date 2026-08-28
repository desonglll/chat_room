use std::sync::{Arc, Mutex};

use axum::{
    extract::State,
    http::{header::CONTENT_TYPE, StatusCode},
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};
use base64::{engine::general_purpose::STANDARD, Engine};
use chat_room::{build_app, config::AppConfig, state::AppState};
use reqwest::{multipart, Client};
use serde_json::{json, Value};
use tokio::net::TcpListener;
use uuid::Uuid;

mod support;
use support::session_token;

#[derive(Default)]
struct ProviderState {
    vision_calls: usize,
    final_prompts: Vec<String>,
}

async fn fake_provider(
    State(state): State<Arc<Mutex<ProviderState>>>,
    Json(payload): Json<Value>,
) -> Response {
    let system = payload["messages"][0]["content"]
        .as_str()
        .unwrap_or_default();
    if system.contains("planning agent") {
        return Json(json!({
            "id": "plan",
            "object": "chat.completion",
            "created": 1,
            "model": "answer-test",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": "{\"intent\":\"overview\",\"context_scope\":\"full\",\"semantic_search\":false,\"research_questions\":[]}"
                },
                "finish_reason": "stop"
            }]
        }))
        .into_response();
    }
    if let Some(parts) = payload["messages"][1]["content"].as_array() {
        let image_url = parts
            .iter()
            .find(|part| part["type"] == "image_url")
            .and_then(|part| part["image_url"]["url"].as_str())
            .unwrap_or_default();
        state.lock().unwrap().vision_calls += 1;
        if image_url.ends_with(&STANDARD.encode(b"provider-failure-image")) {
            return (
                StatusCode::BAD_GATEWAY,
                Json(json!({"error": {"message": "synthetic vision failure"}})),
            )
                .into_response();
        }
        return Json(json!({
            "id": "vision",
            "object": "chat.completion",
            "created": 1,
            "model": "vision-test",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": "{\"summary\":\"Release plan screenshot\",\"visible_text\":[\"Launch Friday\"],\"key_facts\":[\"The launch date is Friday\"],\"uncertainties\":[]}"
                },
                "finish_reason": "stop"
            }]
        }))
        .into_response();
    }
    let prompt = payload["messages"]
        .as_array()
        .and_then(|messages| messages.last())
        .and_then(|message| message["content"].as_str())
        .unwrap_or_default()
        .to_owned();
    state.lock().unwrap().final_prompts.push(prompt);
    let body = concat!(
        "data: {\"id\":\"answer\",\"object\":\"chat.completion.chunk\",\"created\":1,\"model\":\"answer-test\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"The launch is Friday [A1]\"},\"finish_reason\":null}]}\n\n",
        "data: {\"id\":\"answer\",\"object\":\"chat.completion.chunk\",\"created\":1,\"model\":\"answer-test\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n",
        "data: [DONE]\n\n"
    );
    ([(CONTENT_TYPE, "text/event-stream")], body).into_response()
}

#[tokio::test]
async fn visual_pipeline_reuses_cache_binds_context_and_reports_partial_failures() {
    let provider_state = Arc::new(Mutex::new(ProviderState::default()));
    let provider_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let provider_address = provider_listener.local_addr().unwrap();
    let provider_task = tokio::spawn({
        let provider_state = provider_state.clone();
        async move {
            axum::serve(
                provider_listener,
                Router::new()
                    .route("/v1/chat/completions", post(fake_provider))
                    .with_state(provider_state),
            )
            .await
            .unwrap();
        }
    });
    let mut config = AppConfig::default();
    config.ai.enabled = true;
    config.ai.api_key_env = "PATH".into();
    config.ai.model = "answer-test".into();
    config.ai.base_url = Some(format!("http://{provider_address}/v1"));
    config.ai.vision_model = Some("vision-test".into());
    config.ai.vision_base_url = Some(format!("http://{provider_address}/v1"));
    config.ai.vision_api_key_env = "PATH".into();
    config.ai.request_timeout_secs = 5;
    config.ai.stream_idle_timeout_secs = 5;
    config.ai.stream_total_timeout_secs = 20;
    config.ai.vision_request_timeout_secs = 5;
    let state = Arc::new(AppState::new_with_config(&config).await.unwrap());
    let app_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let app_address = app_listener.local_addr().unwrap();
    let app_task = tokio::spawn({
        let state = state.clone();
        async move { axum::serve(app_listener, build_app(state)).await.unwrap() }
    });
    let base = format!("http://{app_address}");
    let client = Client::new();
    let token = session_token(&base, "vision-pipeline-owner").await;
    let room: Value = client
        .post(format!("{base}/api/rooms"))
        .bearer_auth(&token)
        .json(&json!({"name": "Vision pipeline", "join_policy": "open"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let room_id = room["id"].as_str().unwrap();
    upload_image(
        &client,
        &base,
        &token,
        room_id,
        b"successful-image".to_vec(),
        "release.png",
    )
    .await;
    upload_image(
        &client,
        &base,
        &token,
        room_id,
        b"provider-failure-image".to_vec(),
        "unreadable.png",
    )
    .await;

    let first = run_question(&client, &base, &token, room_id).await;
    let first_detail = vision_detail(&first);
    assert!(first_detail.contains("缓存命中 0 张"));
    assert!(first_detail.contains("新提取 1 张"));
    assert!(first_detail.contains("模型失败 1 张"));
    assert_eq!(provider_state.lock().unwrap().vision_calls, 2);

    let second = run_question(&client, &base, &token, room_id).await;
    let second_detail = vision_detail(&second);
    assert!(second_detail.contains("缓存命中 1 张"));
    assert!(second_detail.contains("新提取 0 张"));
    assert!(second_detail.contains("模型失败 1 张"));
    let provider = provider_state.lock().unwrap();
    assert_eq!(provider.vision_calls, 3);
    assert_eq!(provider.final_prompts.len(), 2);
    assert!(provider
        .final_prompts
        .iter()
        .all(|prompt| prompt.contains("source_messages")
            && prompt.contains("The launch date is Friday")));

    app_task.abort();
    provider_task.abort();
}

async fn upload_image(
    client: &Client,
    base: &str,
    token: &str,
    room_id: &str,
    bytes: Vec<u8>,
    file_name: &str,
) {
    let file = multipart::Part::bytes(bytes)
        .file_name(file_name.to_owned())
        .mime_str("image/png")
        .unwrap();
    let response = client
        .post(format!("{base}/api/rooms/{room_id}/attachments"))
        .bearer_auth(token)
        .multipart(
            multipart::Form::new()
                .part("file", file)
                .text("content", file_name.to_owned()),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::CREATED);
}

async fn run_question(client: &Client, base: &str, token: &str, room_id: &str) -> Value {
    let thread: Value = client
        .post(format!("{base}/api/ai/threads"))
        .bearer_auth(token)
        .json(&json!({"room_id": room_id}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let thread_id = thread["id"].as_str().unwrap();
    let response = client
        .post(format!("{base}/api/ai/threads/{thread_id}/runs"))
        .bearer_auth(token)
        .json(&json!({
            "question": "总结所有图片中的发布信息",
            "client_request_id": Uuid::new_v4(),
            "message_ids": []
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::ACCEPTED);
    for _ in 0..100 {
        let messages: Vec<Value> = client
            .get(format!("{base}/api/ai/threads/{thread_id}/messages"))
            .bearer_auth(token)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        if let Some(answer) = messages
            .into_iter()
            .find(|message| message["role"] == "assistant" && message["status"] == "completed")
        {
            return answer;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    panic!("AI answer did not complete");
}

fn vision_detail(answer: &Value) -> &str {
    answer["trace"]
        .as_array()
        .unwrap()
        .iter()
        .find(|step| step["key"] == "vision_context")
        .and_then(|step| step["detail"].as_str())
        .unwrap()
}
