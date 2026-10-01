//! TG-905: `GET /api/admin/access` answers whether the caller is a system administrator with
//! a 200 for every signed-in account, so the client no longer probes a 403 on every load.

use std::sync::Arc;

use chat_room::{
    build_app,
    config::{AdminConfig, AppConfig},
    state::AppState,
};
use tokio::net::TcpListener;

mod support;
use support::{session_token, system_admin_token};

#[tokio::test]
async fn access_answers_yes_or_no_without_a_403() {
    let config = AppConfig {
        admin: AdminConfig {
            usernames: vec!["access-admin".into()],
            ..AdminConfig::default()
        },
        ..AppConfig::default()
    };
    let state = Arc::new(AppState::new_with_config(&config).await.unwrap());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn({
        let state = state.clone();
        async move { axum::serve(listener, build_app(state)).await.unwrap() }
    });
    let client = reqwest::Client::new();
    let regular = session_token(&base, "access-regular").await;
    let admin = system_admin_token(&state, &base, "access-admin").await;

    for (token, expected) in [(&regular, false), (&admin, true)] {
        let response = client
            .get(format!("{base}/api/admin/access"))
            .bearer_auth(token)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        let body: serde_json::Value = response.json().await.unwrap();
        assert_eq!(body["is_admin"], expected);
    }
    let anonymous = client
        .get(format!("{base}/api/admin/access"))
        .send()
        .await
        .unwrap();
    assert_eq!(anonymous.status(), 401);
    task.abort();
}
