use std::sync::Arc;

use axum::{http::StatusCode as AxumStatus, routing::post, Router};
use chat_room::{build_app, config::AppConfig, state::AppState};
use reqwest::{Client, StatusCode};
use tokio::net::TcpListener;
use uuid::Uuid;

pub struct Server {
    pub base: String,
    pub state: Arc<AppState>,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}

pub async fn slow_provider() -> (String, tokio::task::JoinHandle<()>) {
    async fn delayed_failure() -> AxumStatus {
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
        AxumStatus::SERVICE_UNAVAILABLE
    }
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        axum::serve(
            listener,
            Router::new().route("/v1/chat/completions", post(delayed_failure)),
        )
        .await
        .unwrap()
    });
    (format!("http://{address}/v1"), task)
}

pub async fn start(base_url: String) -> Server {
    let mut config = AppConfig::default();
    config.ai.enabled = true;
    config.ai.provider = "openai".into();
    config.ai.model = "gpt-governance-test".into();
    config.ai.api_key_env = "PATH".into();
    config.ai.base_url = Some(base_url);
    let state = Arc::new(AppState::new_with_config(&config).await.unwrap());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn({
        let state = state.clone();
        async move { axum::serve(listener, build_app(state)).await.unwrap() }
    });
    Server { base, state, task }
}

pub async fn create_room(client: &Client, server: &Server, token: &str) -> Uuid {
    let room: serde_json::Value = client
        .post(format!("{}/api/rooms", server.base))
        .bearer_auth(token)
        .json(
            &serde_json::json!({ "name": "AI governance", "password": "", "join_policy": "open" }),
        )
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    Uuid::parse_str(room["id"].as_str().unwrap()).unwrap()
}

pub async fn create_thread(
    client: &Client,
    server: &Server,
    token: &str,
    room_id: Option<Uuid>,
) -> Uuid {
    let thread: serde_json::Value = client
        .post(format!("{}/api/ai/threads", server.base))
        .bearer_auth(token)
        .json(&serde_json::json!({ "room_id": room_id }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    Uuid::parse_str(thread["id"].as_str().unwrap()).unwrap()
}

pub async fn patch_policy(
    client: &Client,
    server: &Server,
    token: &str,
    room_id: Uuid,
    mode: &str,
    version: i64,
) -> reqwest::Response {
    client
        .patch(format!("{}/api/rooms/{room_id}/ai-policy", server.base))
        .bearer_auth(token)
        .json(&serde_json::json!({ "mode": mode, "version": version }))
        .send()
        .await
        .unwrap()
}

pub async fn save_governance(
    client: &Client,
    server: &Server,
    admin: &str,
    max_concurrent: i64,
    user_limit: Option<i64>,
    models_allowed: bool,
) {
    let settings: serde_json::Value = client
        .get(format!("{}/api/admin/ai-governance", server.base))
        .bearer_auth(admin)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let models: Vec<_> = settings["models"]
        .as_array()
        .unwrap()
        .iter()
        .map(|model| {
            serde_json::json!({
                "id": model["id"], "allowed": models_allowed,
                "input_price_micros_per_million": 2_000_000,
                "output_price_micros_per_million": 8_000_000
            })
        })
        .collect();
    let response = client
        .patch(format!("{}/api/admin/ai-governance", server.base))
        .bearer_auth(admin)
        .json(&serde_json::json!({
            "max_concurrent_runs": max_concurrent,
            "daily_user_token_limit": user_limit,
            "daily_room_token_limit": null,
            "allowlist_enabled": true,
            "models": models
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}
