use std::sync::Arc;

use chat_room::{build_app, config::AppConfig, state::AppState};
use redis::AsyncCommands;
use tokio::net::TcpListener;
use uuid::Uuid;

#[tokio::test]
async fn revocation_removes_database_and_redis_session_state() {
    dotenvy::dotenv().ok();
    let Ok(redis_url) =
        std::env::var("TEST_REDIS_URL").or_else(|_| std::env::var("CHAT_ROOM_REDIS_URL"))
    else {
        return;
    };
    let mut config = AppConfig::default();
    config.redis.enabled = true;
    config.redis.url = redis_url.clone();
    config.redis.key_prefix = format!("chat-room-session-test-{}", Uuid::new_v4());
    let state = Arc::new(AppState::new_with_config(&config).await.unwrap());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        axum::serve(listener, build_app(state)).await.unwrap();
    });
    let base = format!("http://{address}");
    let client = reqwest::Client::new();
    let credentials = serde_json::json!({
        "username": "redis-session-user",
        "password": "test-password"
    });
    let registered = client
        .post(format!("{base}/api/users/register"))
        .json(&credentials)
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();
    let old_token = registered["token"].as_str().unwrap().to_owned();
    let user_id = registered["user"]["id"].as_str().unwrap();
    let current_token = client
        .post(format!("{base}/api/users/login"))
        .json(&credentials)
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap()["token"]
        .as_str()
        .unwrap()
        .to_owned();
    let sessions = client
        .get(format!("{base}/api/users/me/sessions"))
        .bearer_auth(&current_token)
        .send()
        .await
        .unwrap()
        .json::<Vec<serde_json::Value>>()
        .await
        .unwrap();
    let old_id = sessions
        .iter()
        .find(|session| session["current"] == false)
        .unwrap()["id"]
        .as_str()
        .unwrap();

    let redis_client = redis::Client::open(redis_url).unwrap();
    let mut redis = redis_client
        .get_multiplexed_async_connection()
        .await
        .unwrap();
    let session_key = format!("{}:session:{old_token}", config.redis.key_prefix);
    let user_key = format!("{}:user-sessions:{user_id}", config.redis.key_prefix);
    assert!(redis.exists::<_, bool>(&session_key).await.unwrap());
    assert_eq!(
        client
            .delete(format!("{base}/api/users/me/sessions/{old_id}"))
            .bearer_auth(&current_token)
            .send()
            .await
            .unwrap()
            .status(),
        204
    );
    assert!(!redis.exists::<_, bool>(&session_key).await.unwrap());
    let members: Vec<String> = redis.smembers(&user_key).await.unwrap();
    assert!(!members.contains(&session_key));
    assert_eq!(
        client
            .get(format!("{base}/api/users/me"))
            .bearer_auth(old_token)
            .send()
            .await
            .unwrap()
            .status(),
        401
    );

    let keys: Vec<String> = redis::cmd("KEYS")
        .arg(format!("{}:*", config.redis.key_prefix))
        .query_async(&mut redis)
        .await
        .unwrap();
    if !keys.is_empty() {
        let _: usize = redis.del(keys).await.unwrap();
    }
    server.abort();
}
