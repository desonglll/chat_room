//! TG-506 acceptance: recovery cannot skip the second factor, and 2FA management
//! (change / disable / recovery email) always demands the current 2FA password.

#[allow(dead_code)]
mod two_factor_support;

use chat_room::state::AppState;
use reqwest::{Client, Method, StatusCode};
use serde_json::{json, Value};
use two_factor_support::*;

async fn recovery(client: &Client, server: &Server, pending: &str) -> reqwest::Response {
    post(
        client,
        server,
        "/api/users/login/two-factor/recovery",
        json!({ "pending_token": pending }),
    )
    .await
}

async fn confirm(client: &Client, server: &Server, pending: &str, code: &str) -> StatusCode {
    post(
        client,
        server,
        "/api/users/login/two-factor/recovery/confirm",
        json!({ "pending_token": pending, "code": code }),
    )
    .await
    .status()
}

#[tokio::test]
async fn recovery_needs_a_pending_token_a_verified_email_and_the_mailed_code() {
    let server = start(AppState::new().await.unwrap()).await;
    let client = Client::new();
    let username = unique("recover");
    let email = format!("{username}@example.com");
    let owner = register(&client, &server, &username).await;
    enable(&client, &server, &owner, "").await;

    // Without a verified address there is nothing to recover through.
    let pending = challenge(&client, &server, &username).await["pending_token"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(
        recovery(&client, &server, &pending).await.status(),
        StatusCode::CONFLICT
    );

    verify_recovery_email(&client, &server, &owner, &email).await;
    let challenge = challenge(&client, &server, &username).await;
    assert_eq!(challenge["has_recovery_email"], true);
    let pending = challenge["pending_token"].as_str().unwrap().to_string();

    // No pending token (i.e. no proven account password) → no recovery at all.
    let invented = uuid::Uuid::new_v4().to_string();
    assert_eq!(
        recovery(&client, &server, &invented).await.status(),
        StatusCode::GONE
    );
    // Confirming before any reset code was mailed gets nothing.
    assert_eq!(
        confirm(&client, &server, &pending, "000000").await,
        StatusCode::GONE
    );
    // A code minted for another purpose (email verification) is not a reset code.
    authed(
        &client,
        &server,
        Method::POST,
        "/api/users/me/two-factor/recovery-email",
        &owner,
        Some(json!({ "current_password": TWO_FA_PASSWORD, "email": email })),
    )
    .await;
    let verify_code = latest_code(&email);
    assert_eq!(
        confirm(&client, &server, &pending, &verify_code).await,
        StatusCode::GONE
    );

    let mailed = recovery(&client, &server, &pending).await;
    assert_eq!(mailed.status(), StatusCode::ACCEPTED);
    let mailed: Value = mailed.json().await.unwrap();
    assert!(mailed["email_pattern"].as_str().unwrap().contains("***@"));
    let code = latest_code(&email);
    let wrong = if code == "999999" { "999998" } else { "999999" };
    assert_eq!(
        confirm(&client, &server, &pending, wrong).await,
        StatusCode::UNAUTHORIZED
    );
    // The reset code is bound to its pending token's account; a foreign token fails.
    assert_eq!(
        confirm(&client, &server, &invented, &code).await,
        StatusCode::GONE
    );
    assert_eq!(me_status(&client, &server, &owner).await, StatusCode::OK);

    let recovered = post(
        &client,
        &server,
        "/api/users/login/two-factor/recovery/confirm",
        json!({ "pending_token": pending, "code": code }),
    )
    .await;
    assert_eq!(recovered.status(), StatusCode::OK);
    let recovered = token_of(recovered).await;

    // The reset removed 2FA and ended every earlier session.
    assert_eq!(
        me_status(&client, &server, &owner).await,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        me_status(&client, &server, &recovered).await,
        StatusCode::OK
    );
    assert_eq!(
        login(&client, &server, &username).await.status(),
        StatusCode::OK
    );
    // The code and the pending token are both spent.
    assert_eq!(
        confirm(&client, &server, &pending, &code).await,
        StatusCode::GONE
    );
}

#[tokio::test]
async fn change_and_disable_demand_the_current_two_factor_password() {
    let server = start(AppState::new().await.unwrap()).await;
    let client = Client::new();
    let username = unique("manage");
    let token = register(&client, &server, &username).await;
    let second_device = token_of(login(&client, &server, &username).await).await;
    let enabled: Value = enable(&client, &server, &token, "old hint")
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(enabled["enabled"], true);
    assert_eq!(
        me_status(&client, &server, &second_device).await,
        StatusCode::UNAUTHORIZED
    );

    let put = |body: Value| {
        authed(
            &client,
            &server,
            Method::PUT,
            "/api/users/me/two-factor",
            &token,
            Some(body),
        )
    };
    assert_eq!(
        put(json!({ "current_password": "wrong-second-pw", "new_password": "new-second-pw" }))
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    let changed = put(json!({
        "current_password": TWO_FA_PASSWORD,
        "new_password": "new-second-pw",
        "hint": "new hint",
    }))
    .await;
    assert_eq!(changed.status(), StatusCode::OK);
    assert_eq!(changed.json::<Value>().await.unwrap()["hint"], "new hint");
    // Changing keeps the current session.
    assert_eq!(me_status(&client, &server, &token).await, StatusCode::OK);

    let challenge = challenge(&client, &server, &username).await;
    assert_eq!(challenge["hint"], "new hint");
    let pending = challenge["pending_token"].as_str().unwrap();
    assert_eq!(
        second_stage(&client, &server, pending, TWO_FA_PASSWORD)
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        second_stage(&client, &server, pending, "new-second-pw")
            .await
            .status(),
        StatusCode::OK
    );

    let delete = |password: &str| {
        authed(
            &client,
            &server,
            Method::DELETE,
            "/api/users/me/two-factor",
            &token,
            Some(json!({ "current_password": password })),
        )
    };
    assert_eq!(
        delete(TWO_FA_PASSWORD).await.status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        delete("new-second-pw").await.status(),
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        delete("new-second-pw").await.status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        login(&client, &server, &username).await.status(),
        StatusCode::OK
    );
}

#[tokio::test]
async fn a_recovery_email_needs_the_two_factor_password_and_its_code() {
    outbox();
    let server = start(AppState::new().await.unwrap()).await;
    let client = Client::new();
    let username = unique("mailbox");
    let email = format!("{username}@example.com");
    let token = register(&client, &server, &username).await;
    let request = |password: &str, address: &str| {
        authed(
            &client,
            &server,
            Method::POST,
            "/api/users/me/two-factor/recovery-email",
            &token,
            Some(json!({ "current_password": password, "email": address })),
        )
    };
    // 2FA off: there is nothing to attach an address to.
    assert_eq!(
        request(TWO_FA_PASSWORD, &email).await.status(),
        StatusCode::NOT_FOUND
    );
    enable(&client, &server, &token, "").await;
    assert_eq!(
        request("wrong-second-pw", &email).await.status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        request(TWO_FA_PASSWORD, "not-an-email").await.status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        request(TWO_FA_PASSWORD, &email).await.status(),
        StatusCode::ACCEPTED
    );
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
    assert_eq!(status["pending_recovery_email"], email.as_str());
    assert_eq!(status["recovery_email"], Value::Null);

    let code = latest_code(&email);
    let wrong = if code == "111111" { "111112" } else { "111111" };
    let confirm = |code: &str| {
        authed(
            &client,
            &server,
            Method::POST,
            "/api/users/me/two-factor/recovery-email/confirm",
            &token,
            Some(json!({ "code": code })),
        )
    };
    assert_eq!(confirm(wrong).await.status(), StatusCode::UNAUTHORIZED);
    let confirmed = confirm(&code).await;
    assert_eq!(confirmed.status(), StatusCode::OK);
    let status: Value = confirmed.json().await.unwrap();
    assert_eq!(status["recovery_email"], email.as_str());
    assert_eq!(status["pending_recovery_email"], Value::Null);
    assert_eq!(confirm(&code).await.status(), StatusCode::GONE);
}
