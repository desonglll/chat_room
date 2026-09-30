//! TG-506: this server ships without a mail transport. With none installed, asking for a
//! recovery-email code is an honest `503` and leaves nothing pending. (A separate test
//! binary on purpose: the other TG-506 binaries install an in-memory outbox process-wide.)

#[allow(dead_code)]
mod two_factor_support;

use chat_room::state::AppState;
use reqwest::{Client, Method, StatusCode};
use serde_json::{json, Value};
use two_factor_support::*;

#[tokio::test]
async fn recovery_email_is_unavailable_without_a_mail_transport() {
    let server = start(AppState::new().await.unwrap()).await;
    let client = Client::new();
    let token = register(&client, &server, &unique("no-mail")).await;
    enable(&client, &server, &token, "").await;
    let response = authed(
        &client,
        &server,
        Method::POST,
        "/api/users/me/two-factor/recovery-email",
        &token,
        Some(json!({ "current_password": TWO_FA_PASSWORD, "email": "someone@example.com" })),
    )
    .await;
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let status: Value = authed(
        &client,
        &server,
        Method::GET,
        "/api/users/me/two-factor",
        &token,
        None,
    )
    .await
    .json()
    .await
    .unwrap();
    assert_eq!(status["pending_recovery_email"], Value::Null);
    assert_eq!(status["recovery_email"], Value::Null);
}
