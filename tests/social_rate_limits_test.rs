use std::sync::Arc;

use chat_room::{build_app, state::AppState};
use reqwest::{Client, StatusCode};
use tokio::net::TcpListener;

async fn register(client: &Client, base: &str, username: &str) -> (String, String) {
    let body = client
        .post(format!("{base}/api/users/register"))
        .json(&serde_json::json!({ "username": username, "password": "test-password" }))
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();
    (
        body["user"]["id"].as_str().unwrap().to_string(),
        body["token"].as_str().unwrap().to_string(),
    )
}

#[tokio::test]
async fn social_search_and_new_pair_mutations_are_rate_limited() {
    let state = Arc::new(AppState::new().await.unwrap());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        axum::serve(listener, build_app(state)).await.unwrap();
    });
    let client = Client::new();
    let (_, alice_token) = register(&client, &base, "limit-alice").await;
    let (bob_id, _) = register(&client, &base, "limit-bob").await;

    for _ in 0..30 {
        assert_eq!(
            client
                .get(format!("{base}/api/users/search?q=limit"))
                .bearer_auth(&alice_token)
                .send()
                .await
                .unwrap()
                .status(),
            StatusCode::OK
        );
    }
    assert_eq!(
        client
            .get(format!("{base}/api/users/search?q=limit"))
            .bearer_auth(&alice_token)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::TOO_MANY_REQUESTS
    );

    let request = || {
        client
            .post(format!("{base}/api/friend-requests"))
            .bearer_auth(&alice_token)
            .json(&serde_json::json!({ "user_id": bob_id }))
            .send()
    };
    assert_eq!(request().await.unwrap().status(), StatusCode::CREATED);
    assert_eq!(request().await.unwrap().status(), StatusCode::OK);
    client
        .delete(format!("{base}/api/friend-requests/{bob_id}"))
        .bearer_auth(&alice_token)
        .send()
        .await
        .unwrap();
    assert_eq!(
        request().await.unwrap().status(),
        StatusCode::TOO_MANY_REQUESTS
    );
    task.abort();
}
