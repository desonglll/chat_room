use std::{sync::Arc, time::Duration};

use axum::{http::StatusCode, Router};
use chat_room::{build_app, config::AppConfig, state::AppState};
use reqwest::{header, Client, Response};
use serde_json::json;
use uuid::Uuid;

struct Server {
    base: String,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn start(app: Router) -> Server {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    Server {
        base: format!("http://{address}"),
        task,
    }
}

async fn auth(client: &Client, server: &Server, route: &str, username: &str) -> Response {
    client
        .post(format!("{}/api/users/{route}", server.base))
        .header("x-forwarded-for", "203.0.113.10")
        .json(&json!({ "username": username, "password": "password-123" }))
        .send()
        .await
        .unwrap()
}

fn limited_config() -> AppConfig {
    let mut config = AppConfig::default();
    config.auth.rate_limit_window_secs = 60;
    config.auth.rate_limit_ip_attempts = 20;
    config.auth.rate_limit_account_attempts = 2;
    config.security.trust_proxy_headers = true;
    config
}

#[tokio::test]
async fn authentication_routes_enforce_account_limits_before_password_work() {
    let state = Arc::new(AppState::new_with_config(&limited_config()).await.unwrap());
    let server = start(build_app(state)).await;
    let client = Client::new();

    let registration = auth(&client, &server, "register", "limited-user").await;
    assert_eq!(registration.status(), 201);
    let session = registration.json::<serde_json::Value>().await.unwrap();
    let token = session["token"].as_str().unwrap().to_string();
    assert_eq!(
        auth(&client, &server, "register", "limited-user")
            .await
            .status(),
        409
    );
    assert_eq!(
        auth(&client, &server, "register", "limited-user")
            .await
            .status(),
        429
    );

    for expected in [
        StatusCode::UNAUTHORIZED,
        StatusCode::UNAUTHORIZED,
        StatusCode::TOO_MANY_REQUESTS,
    ] {
        let response = client
            .post(format!("{}/api/users/login", server.base))
            .header("x-forwarded-for", "203.0.113.11")
            .json(&json!({ "username": "limited-user", "password": "wrong-pass" }))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), expected);
    }

    for expected in [
        StatusCode::UNAUTHORIZED,
        StatusCode::UNAUTHORIZED,
        StatusCode::TOO_MANY_REQUESTS,
    ] {
        let response = client
            .post(format!("{}/api/users/me/verify-password", server.base))
            .header("x-forwarded-for", "203.0.113.12")
            .bearer_auth(&token)
            .json(&json!({ "current_password": "wrong-pass" }))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), expected);
    }
}

#[tokio::test]
async fn login_failure_does_not_reveal_account_existence() {
    let state = Arc::new(AppState::new().await.unwrap());
    let server = start(build_app(state)).await;
    let client = Client::new();
    assert_eq!(
        auth(&client, &server, "register", "known-user")
            .await
            .status(),
        201
    );

    let missing = auth(&client, &server, "login", "missing-user").await;
    let wrong = client
        .post(format!("{}/api/users/login", server.base))
        .json(&json!({ "username": "known-user", "password": "wrong-pass" }))
        .send()
        .await
        .unwrap();
    assert_eq!(missing.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(wrong.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(missing.bytes().await.unwrap(), wrong.bytes().await.unwrap());
}

#[tokio::test]
async fn responses_include_security_headers_and_cors_is_exact() {
    let mut config = AppConfig::default();
    config.security.cors_allowed_origins = vec!["https://chat.example.com".into()];
    let state = Arc::new(AppState::new_with_config(&config).await.unwrap());
    let server = start(build_app(state)).await;
    let client = Client::new();

    let response = client
        .get(format!("{}/api/config", server.base))
        .send()
        .await
        .unwrap();
    let headers = response.headers();
    assert!(headers["content-security-policy"]
        .to_str()
        .unwrap()
        .contains("frame-ancestors 'none'"));
    assert_eq!(headers["x-frame-options"], "DENY");
    assert_eq!(headers["x-content-type-options"], "nosniff");
    assert_eq!(
        headers["referrer-policy"],
        "strict-origin-when-cross-origin"
    );

    let allowed = client
        .request(
            reqwest::Method::OPTIONS,
            format!("{}/api/users/login", server.base),
        )
        .header(header::ORIGIN, "https://chat.example.com")
        .header(header::ACCESS_CONTROL_REQUEST_METHOD, "POST")
        .send()
        .await
        .unwrap();
    assert_eq!(
        allowed.headers()[header::ACCESS_CONTROL_ALLOW_ORIGIN],
        "https://chat.example.com"
    );

    let rejected = client
        .request(
            reqwest::Method::OPTIONS,
            format!("{}/api/users/login", server.base),
        )
        .header(header::ORIGIN, "https://evil.example.com")
        .header(header::ACCESS_CONTROL_REQUEST_METHOD, "POST")
        .send()
        .await
        .unwrap();
    assert!(!rejected
        .headers()
        .contains_key(header::ACCESS_CONTROL_ALLOW_ORIGIN));
}

#[tokio::test]
async fn redis_adapter_shares_limits_between_instances_when_configured() {
    dotenvy::dotenv().ok();
    let Ok(redis_url) =
        std::env::var("TEST_REDIS_URL").or_else(|_| std::env::var("CHAT_ROOM_REDIS_URL"))
    else {
        return;
    };
    let mut config = limited_config();
    config.auth.rate_limit_window_secs = 2;
    config.redis.enabled = true;
    config.redis.url = redis_url;
    config.redis.key_prefix = format!("chat-room-test-{}", Uuid::new_v4());
    let first = start(build_app(Arc::new(
        AppState::new_with_config(&config).await.unwrap(),
    )))
    .await;
    let second = start(build_app(Arc::new(
        AppState::new_with_config(&config).await.unwrap(),
    )))
    .await;
    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .unwrap();

    assert_eq!(
        auth(&client, &first, "login", "shared-limit")
            .await
            .status(),
        401
    );
    assert_eq!(
        auth(&client, &second, "login", "shared-limit")
            .await
            .status(),
        401
    );
    assert_eq!(
        auth(&client, &first, "login", "shared-limit")
            .await
            .status(),
        429
    );
}
