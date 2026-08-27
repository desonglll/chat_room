pub async fn session_token(base: &str, username: &str) -> String {
    let client = reqwest::Client::new();
    let credentials = serde_json::json!({ "username": username, "password": "test-password" });
    let mut response = client
        .post(format!("{base}/api/users/register"))
        .json(&credentials)
        .send()
        .await
        .unwrap();
    if response.status() == 409 {
        response = client
            .post(format!("{base}/api/users/login"))
            .json(&credentials)
            .send()
            .await
            .unwrap();
    }
    assert!(response.status().is_success());
    response.json::<serde_json::Value>().await.unwrap()["token"]
        .as_str()
        .unwrap()
        .to_string()
}

#[allow(dead_code)]
pub async fn system_admin_token(
    state: &chat_room::state::AppState,
    base: &str,
    username: &str,
) -> String {
    let token = session_token(base, username).await;
    let user = state
        .session_user(uuid::Uuid::parse_str(&token).unwrap())
        .await
        .unwrap()
        .unwrap();
    if !state.is_system_admin(user.id).await.unwrap() {
        state.bootstrap_system_admin(username).await.unwrap();
    }
    token
}
