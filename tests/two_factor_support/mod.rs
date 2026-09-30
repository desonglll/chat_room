//! Shared harness for the TG-506 two-factor tests: a server on any `AppState`, a
//! process-wide in-memory mail outbox, and thin HTTP helpers.

use std::sync::{Arc, OnceLock};

use chat_room::{
    accounts::two_factor::{install_recovery_mailer, MemoryMailer},
    build_app,
    state::AppState,
};
use reqwest::{Client, Response, StatusCode};
use serde_json::{json, Value};

pub const ACCOUNT_PASSWORD: &str = "account-password";
pub const TWO_FA_PASSWORD: &str = "second-factor-pw";

pub struct Server {
    pub base: String,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}

pub async fn start(state: AppState) -> Server {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let app = build_app(Arc::new(state));
    let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    Server {
        base: format!("http://{address}"),
        task,
    }
}

/// The one outbox of this test binary; installed on first use.
pub fn outbox() -> Arc<MemoryMailer> {
    static OUTBOX: OnceLock<Arc<MemoryMailer>> = OnceLock::new();
    OUTBOX
        .get_or_init(|| {
            let mailer = Arc::new(MemoryMailer::default());
            install_recovery_mailer(mailer.clone());
            mailer
        })
        .clone()
}

pub fn latest_code(email: &str) -> String {
    outbox()
        .take_latest(email)
        .unwrap_or_else(|| panic!("no mail for {email}"))
        .code
}

pub fn unique(label: &str) -> String {
    format!(
        "{label}-{}",
        &uuid::Uuid::new_v4().simple().to_string()[..8]
    )
}

pub async fn post(client: &Client, server: &Server, path: &str, body: Value) -> Response {
    client
        .post(format!("{}{path}", server.base))
        .json(&body)
        .send()
        .await
        .unwrap()
}

pub async fn authed(
    client: &Client,
    server: &Server,
    method: reqwest::Method,
    path: &str,
    token: &str,
    body: Option<Value>,
) -> Response {
    let mut request = client
        .request(method, format!("{}{path}", server.base))
        .bearer_auth(token);
    if let Some(body) = body {
        request = request.json(&body);
    }
    request.send().await.unwrap()
}

pub async fn register(client: &Client, server: &Server, username: &str) -> String {
    let response = post(
        client,
        server,
        "/api/users/register",
        json!({ "username": username, "password": ACCOUNT_PASSWORD }),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    token_of(response).await
}

pub async fn token_of(response: Response) -> String {
    response.json::<Value>().await.unwrap()["token"]
        .as_str()
        .unwrap()
        .to_string()
}

pub async fn login(client: &Client, server: &Server, username: &str) -> Response {
    post(
        client,
        server,
        "/api/users/login",
        json!({ "username": username, "password": ACCOUNT_PASSWORD }),
    )
    .await
}

/// Stage one for an account with 2FA: returns the `428` body.
pub async fn challenge(client: &Client, server: &Server, username: &str) -> Value {
    let response = login(client, server, username).await;
    assert_eq!(response.status(), StatusCode::PRECONDITION_REQUIRED);
    response.json().await.unwrap()
}

pub async fn second_stage(
    client: &Client,
    server: &Server,
    pending_token: &str,
    password: &str,
) -> Response {
    post(
        client,
        server,
        "/api/users/login/two-factor",
        json!({ "pending_token": pending_token, "password": password }),
    )
    .await
}

pub async fn enable(client: &Client, server: &Server, token: &str, hint: &str) -> Response {
    authed(
        client,
        server,
        reqwest::Method::POST,
        "/api/users/me/two-factor",
        token,
        Some(json!({
            "account_password": ACCOUNT_PASSWORD,
            "password": TWO_FA_PASSWORD,
            "hint": hint,
        })),
    )
    .await
}

pub async fn me_status(client: &Client, server: &Server, token: &str) -> StatusCode {
    authed(
        client,
        server,
        reqwest::Method::GET,
        "/api/users/me",
        token,
        None,
    )
    .await
    .status()
}

/// Request and confirm a recovery address through the outbox.
pub async fn verify_recovery_email(client: &Client, server: &Server, token: &str, email: &str) {
    outbox();
    let requested = authed(
        client,
        server,
        reqwest::Method::POST,
        "/api/users/me/two-factor/recovery-email",
        token,
        Some(json!({ "current_password": TWO_FA_PASSWORD, "email": email })),
    )
    .await;
    assert_eq!(requested.status(), StatusCode::ACCEPTED);
    let code = latest_code(email);
    let confirmed = authed(
        client,
        server,
        reqwest::Method::POST,
        "/api/users/me/two-factor/recovery-email/confirm",
        token,
        Some(json!({ "code": code })),
    )
    .await;
    assert_eq!(confirmed.status(), StatusCode::OK);
    let status: Value = confirmed.json().await.unwrap();
    assert_eq!(status["recovery_email"], email);
}
