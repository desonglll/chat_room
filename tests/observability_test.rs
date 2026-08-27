use std::sync::Arc;

use chat_room::{build_app, config::AppConfig, state::AppState};
use reqwest::StatusCode;
use tokio::net::TcpListener;

async fn start(state: Arc<AppState>) -> (String, tokio::task::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        axum::serve(listener, build_app(state)).await.unwrap();
    });
    (format!("http://{address}"), task)
}

#[tokio::test]
async fn liveness_readiness_metrics_and_request_ids_are_privacy_safe() {
    let state = Arc::new(AppState::new().await.unwrap());
    let (base, task) = start(state.clone()).await;
    let client = reqwest::Client::new();

    let live = client
        .get(format!("{base}/health/live"))
        .header("x-request-id", "deployment-probe-42")
        .send()
        .await
        .unwrap();
    assert_eq!(live.status(), StatusCode::OK);
    assert_eq!(live.headers()["x-request-id"], "deployment-probe-42");
    assert_eq!(
        live.json::<serde_json::Value>().await.unwrap()["status"],
        "live"
    );
    let generated = client
        .get(format!("{base}/health/live"))
        .header(
            "x-request-id",
            "private message must not become a request id",
        )
        .send()
        .await
        .unwrap();
    uuid::Uuid::parse_str(generated.headers()["x-request-id"].to_str().unwrap()).unwrap();

    let ready = client
        .get(format!("{base}/health/ready"))
        .send()
        .await
        .unwrap();
    assert_eq!(ready.status(), StatusCode::OK);
    let ready = ready.json::<serde_json::Value>().await.unwrap();
    assert_eq!(ready["status"], "ready");
    assert_eq!(ready["dependencies"][0]["id"], "database");
    assert_eq!(ready["dependencies"][0]["required"], true);

    let metrics = client.get(format!("{base}/metrics")).send().await.unwrap();
    assert_eq!(metrics.status(), StatusCode::OK);
    assert!(metrics.headers()["content-type"]
        .to_str()
        .unwrap()
        .starts_with("text/plain"));
    let body = metrics.text().await.unwrap();
    assert!(body.contains("chat_room_http_requests_total"));
    assert!(body.contains("chat_room_dependency_up{dependency=\"database\",required=\"true\"} 1"));
    for private in ["deployment-probe-42", "/health/ready", "room_id", "user_id"] {
        assert!(!body.contains(private));
    }

    state.pool().close().await;
    assert_eq!(
        client
            .get(format!("{base}/health/ready"))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(
        client
            .get(format!("{base}/health/live"))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    task.abort();
}

#[tokio::test]
async fn readiness_distinguishes_optional_and_required_degradation() {
    for (required, expected_status, expected_state) in [
        (false, StatusCode::OK, "degraded"),
        (true, StatusCode::SERVICE_UNAVAILABLE, "not_ready"),
    ] {
        let mut config = AppConfig::default();
        config.redis.enabled = true;
        config.redis.url = "redis://127.0.0.1:1/".into();
        config.redis.connect_timeout_ms = 10;
        config.observability.required_dependencies = if required {
            vec!["redis".into()]
        } else {
            Vec::new()
        };
        let state = Arc::new(AppState::new_with_config(&config).await.unwrap());
        let (base, task) = start(state).await;
        let response = reqwest::get(format!("{base}/health/ready")).await.unwrap();
        assert_eq!(response.status(), expected_status);
        let body = response.json::<serde_json::Value>().await.unwrap();
        assert_eq!(body["status"], expected_state);
        let redis = body["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .find(|dependency| dependency["id"] == "redis")
            .unwrap();
        assert_eq!(redis["required"], required);
        assert_eq!(redis["status"], "degraded");
        task.abort();
    }
}

#[test]
fn container_healthcheck_uses_readiness() {
    let dockerfile = include_str!("../Dockerfile");
    assert!(dockerfile.contains("http://127.0.0.1:3000/health/ready"));
    assert!(!dockerfile.contains("HEALTHCHECK") || !dockerfile.contains("3000/api/config"));
}
