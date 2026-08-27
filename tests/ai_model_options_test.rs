use std::sync::Arc;

use chat_room::{
    build_app,
    config::{AdminConfig, AppConfig},
    state::AppState,
};
use reqwest::{Client, StatusCode};
use tokio::net::TcpListener;

mod support;
use support::{session_token, system_admin_token};

#[tokio::test]
async fn admins_manage_model_options_and_users_select_only_enabled_models() {
    let config = AppConfig {
        admin: AdminConfig {
            usernames: vec!["model-admin".into()],
            ..AdminConfig::default()
        },
        ..AppConfig::default()
    };
    let state = Arc::new(AppState::new_with_config(&config).await.unwrap());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn({
        let state = state.clone();
        async move { axum::serve(listener, build_app(state)).await.unwrap() }
    });
    let client = Client::new();
    let admin = system_admin_token(&state, &base, "model-admin").await;
    let regular = session_token(&base, "model-regular").await;

    assert_eq!(
        client
            .get(format!("{base}/api/admin/ai-models"))
            .bearer_auth(&regular)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );

    let created: serde_json::Value = client
        .post(format!("{base}/api/admin/ai-models"))
        .bearer_auth(&admin)
        .json(&serde_json::json!({
            "label": "Internal gateway",
            "provider": "openai",
            "base_url": "http://127.0.0.1:9000/v1",
            "model": "model-a",
            "api_key_env": "PATH",
            "enabled": true
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(created["ready"], true);
    let id = created["id"].as_str().unwrap();

    let choices: Vec<serde_json::Value> = client
        .get(format!("{base}/api/ai/models"))
        .bearer_auth(&regular)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(choices.iter().any(|choice| choice["id"] == id));

    let updated: serde_json::Value = client
        .put(format!("{base}/api/admin/ai-models/{id}"))
        .bearer_auth(&admin)
        .json(&serde_json::json!({
            "label": "Internal gateway",
            "provider": "openai",
            "base_url": "http://127.0.0.1:9000/v1",
            "model": "model-b",
            "api_key_env": "PATH",
            "enabled": false
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(updated["enabled"], false);

    let choices: Vec<serde_json::Value> = client
        .get(format!("{base}/api/ai/models"))
        .bearer_auth(&regular)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(!choices.iter().any(|choice| choice["id"] == id));

    assert_eq!(
        client
            .delete(format!("{base}/api/admin/ai-models/{id}"))
            .bearer_auth(&admin)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::NO_CONTENT
    );
    server.abort();
}
