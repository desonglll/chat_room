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
