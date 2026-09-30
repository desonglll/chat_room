//! TG-506 acceptance: two-stage login, rate limiting of wrong 2FA passwords, and the
//! D-010 rule that enabling 2FA ends every other session but keeps the current one.

#[allow(dead_code)]
mod two_factor_support;

use chat_room::{config::AppConfig, state::AppState};
use reqwest::{Client, Method, StatusCode};
use serde_json::Value;
use two_factor_support::*;

#[tokio::test]
async fn accounts_without_two_factor_keep_the_frozen_login_response() {
    let server = start(AppState::new().await.unwrap()).await;
    let client = Client::new();
    let username = unique("plain");
    register(&client, &server, &username).await;

    let response = login(&client, &server, &username).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = response.json().await.unwrap();
    let mut keys: Vec<&str> = body
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(keys, ["expires_at", "token", "user"]);
}

#[tokio::test]
async fn login_is_two_stage_once_two_factor_is_on() {
    let server = start(AppState::new().await.unwrap()).await;
    let client = Client::new();
    let username = unique("two-stage");
    let token = register(&client, &server, &username).await;
    assert_eq!(
        enable(&client, &server, &token, "favourite film")
            .await
            .status(),
        StatusCode::OK
    );

    // A wrong account password never reaches the second stage.
    let wrong = post(
        &client,
        &server,
        "/api/users/login",
        serde_json::json!({ "username": username, "password": "not-the-password" }),
    )
    .await;
    assert_eq!(wrong.status(), StatusCode::UNAUTHORIZED);

    let challenge = challenge(&client, &server, &username).await;
    assert_eq!(challenge["error"], "two_factor_required");
    assert_eq!(challenge["hint"], "favourite film");
    assert_eq!(challenge["has_recovery_email"], false);
    assert!(
        challenge.get("token").is_none(),
        "stage one issues no session"
    );
    let pending = challenge["pending_token"].as_str().unwrap().to_string();

    // The pending token is not a session.
    assert_eq!(
        me_status(&client, &server, &pending).await,
        StatusCode::UNAUTHORIZED
    );

    assert_eq!(
        second_stage(&client, &server, &pending, "wrong-second-pw")
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    let accepted = second_stage(&client, &server, &pending, TWO_FA_PASSWORD).await;
    assert_eq!(accepted.status(), StatusCode::OK);
    let session: Value = accepted.json().await.unwrap();
    let session_token = session["token"].as_str().unwrap();
    assert_eq!(session["user"]["username"], username.as_str());
    assert_eq!(
        me_status(&client, &server, session_token).await,
        StatusCode::OK
    );

    // A pending token is single-use.
    assert_eq!(
        second_stage(&client, &server, &pending, TWO_FA_PASSWORD)
            .await
            .status(),
        StatusCode::GONE
    );
    // And an invented one is refused.
    assert_eq!(
        second_stage(
            &client,
            &server,
            &uuid::Uuid::new_v4().to_string(),
            TWO_FA_PASSWORD
        )
        .await
        .status(),
        StatusCode::GONE
    );
}

#[tokio::test]
async fn a_pending_token_dies_after_five_wrong_passwords() {
    let server = start(AppState::new().await.unwrap()).await;
    let client = Client::new();
    let username = unique("exhaust");
    let token = register(&client, &server, &username).await;
    enable(&client, &server, &token, "").await;
    let pending = challenge(&client, &server, &username).await["pending_token"]
        .as_str()
        .unwrap()
        .to_string();
    for _ in 0..5 {
        assert_eq!(
            second_stage(&client, &server, &pending, "wrong-second-pw")
                .await
                .status(),
            StatusCode::UNAUTHORIZED
        );
    }
    // Even the right password cannot revive an exhausted challenge.
    assert_eq!(
        second_stage(&client, &server, &pending, TWO_FA_PASSWORD)
            .await
            .status(),
        StatusCode::GONE
    );
}

#[tokio::test]
async fn wrong_two_factor_passwords_are_rate_limited_per_account() {
    let mut config = AppConfig::default();
    config.auth.rate_limit_window_secs = 60;
    config.auth.rate_limit_ip_attempts = 100;
    config.auth.rate_limit_account_attempts = 3;
    let server = start(AppState::new_with_config(&config).await.unwrap()).await;
    let client = Client::new();
    let username = unique("limited");
    let token = register(&client, &server, &username).await;
    // Enabling spends one `two-factor` attempt for this account.
    assert_eq!(
        enable(&client, &server, &token, "").await.status(),
        StatusCode::OK
    );

    // Each stage-one login uses a fresh challenge, so only the limiter can stop guessing.
    let mut statuses = Vec::new();
    for _ in 0..2 {
        let pending = challenge(&client, &server, &username).await["pending_token"]
            .as_str()
            .unwrap()
            .to_string();
        statuses.push(
            second_stage(&client, &server, &pending, "wrong-second-pw")
                .await
                .status(),
        );
    }
    assert_eq!(statuses, [StatusCode::UNAUTHORIZED; 2]);
    let pending = challenge(&client, &server, &username).await["pending_token"]
        .as_str()
        .unwrap()
        .to_string();
    // The limiter answers before any password work: even the right password is refused.
    assert_eq!(
        second_stage(&client, &server, &pending, TWO_FA_PASSWORD)
            .await
            .status(),
        StatusCode::TOO_MANY_REQUESTS
    );
}

#[tokio::test]
async fn enabling_two_factor_ends_every_other_session_but_keeps_the_current_one() {
    let server = start(AppState::new().await.unwrap()).await;
    let client = Client::new();
    let username = unique("sessions");
    let first = register(&client, &server, &username).await;
    let second = token_of(login(&client, &server, &username).await).await;
    let current = token_of(login(&client, &server, &username).await).await;

    assert_eq!(
        enable(&client, &server, &current, "").await.status(),
        StatusCode::OK
    );
    assert_eq!(
        me_status(&client, &server, &first).await,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        me_status(&client, &server, &second).await,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(me_status(&client, &server, &current).await, StatusCode::OK);
    let sessions: Vec<Value> = authed(
        &client,
        &server,
        Method::GET,
        "/api/users/me/sessions",
        &current,
        None,
    )
    .await
    .json()
    .await
    .unwrap();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0]["current"], true);

    // A second enable is refused and ends nothing.
    let again = token_of({
        let challenge = challenge(&client, &server, &username).await;
        second_stage(
            &client,
            &server,
            challenge["pending_token"].as_str().unwrap(),
            TWO_FA_PASSWORD,
        )
        .await
    })
    .await;
    assert_eq!(
        enable(&client, &server, &again, "").await.status(),
        StatusCode::CONFLICT
    );
    assert_eq!(me_status(&client, &server, &current).await, StatusCode::OK);
}

#[tokio::test]
async fn enabling_requires_the_account_password_and_a_safe_hint() {
    let server = start(AppState::new().await.unwrap()).await;
    let client = Client::new();
    let token = register(&client, &server, &unique("guarded")).await;
    let other = login(&client, &server, "nobody-here").await;
    assert_eq!(other.status(), StatusCode::UNAUTHORIZED);

    let wrong_account = authed(
        &client,
        &server,
        Method::POST,
        "/api/users/me/two-factor",
        &token,
        Some(serde_json::json!({
            "account_password": "stolen-session-guess",
            "password": TWO_FA_PASSWORD,
            "hint": "",
        })),
    )
    .await;
    assert_eq!(wrong_account.status(), StatusCode::UNAUTHORIZED);
    let revealing = enable(&client, &server, &token, "hint: second-factor-pw").await;
    assert_eq!(revealing.status(), StatusCode::BAD_REQUEST);
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
    assert_eq!(status["enabled"], false);
}
