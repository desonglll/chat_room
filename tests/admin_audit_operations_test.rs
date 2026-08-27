mod support;

use std::sync::Arc;

use chat_room::{
    build_app,
    config::{AppConfig, VectorStoreConfig},
    state::AppState,
};
use tokio::net::TcpListener;

#[tokio::test]
async fn model_index_and_cleanup_operations_are_privacy_safe_audit_events() {
    let config = AppConfig {
        vector_store: VectorStoreConfig {
            enabled: true,
            url: "http://127.0.0.1:9".into(),
            collection: "audit-test".into(),
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
    let address = listener.local_addr().unwrap();
    let app = build_app(state.clone());
    let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let base = format!("http://{address}");
    let client = reqwest::Client::new();
    let admin_token = support::system_admin_token(&state, &base, "operations-admin").await;
    let payload = serde_json::json!({
        "label": "Private endpoint",
        "provider": "openai",
        "base_url": "https://private.example.test/v1",
        "model": "private-model",
        "api_key_env": "PRIVATE_AI_SECRET",
        "enabled": true
    });
    let created = client
        .post(format!("{base}/api/admin/ai-models"))
        .bearer_auth(&admin_token)
        .json(&payload)
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();
    let model_id = created["id"].as_str().unwrap();
    assert_eq!(
        client
            .put(format!("{base}/api/admin/ai-models/{model_id}"))
            .bearer_auth(&admin_token)
            .json(&payload)
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    assert_eq!(
        client
            .delete(format!("{base}/api/admin/ai-models/{model_id}"))
            .bearer_auth(&admin_token)
            .send()
            .await
            .unwrap()
            .status(),
        204
    );
    assert_eq!(
        client
            .post(format!("{base}/api/admin/indexes/sync"))
            .bearer_auth(&admin_token)
            .json(&serde_json::json!({ "target": "vector" }))
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    assert_eq!(
        client
            .post(format!("{base}/api/admin/maintenance/purge"))
            .bearer_auth(&admin_token)
            .send()
            .await
            .unwrap()
            .status(),
        200
    );

    let page = client
        .get(format!("{base}/api/admin/audit-events?limit=100"))
        .bearer_auth(&admin_token)
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();
    let serialized = serde_json::to_string(&page).unwrap();
    for sensitive in ["private.example.test", "PRIVATE_AI_SECRET", "private-model"] {
        assert!(!serialized.contains(sensitive));
    }
    let event_types = page["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|event| event["event_type"].as_str().unwrap())
        .collect::<std::collections::HashSet<_>>();
    for expected in [
        "ai_model.create_requested",
        "ai_model.update_requested",
        "ai_model.delete_requested",
        "index.rebuild_requested",
        "retention.purge_requested",
    ] {
        assert!(event_types.contains(expected), "missing {expected}");
    }

    task.abort();
}
