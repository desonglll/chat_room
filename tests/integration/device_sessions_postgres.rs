use super::*;
use chat_room::config::AppConfig;

#[tokio::test]
async fn postgres_lists_and_revokes_device_sessions() {
    let Some((admin_url, admin_pool)) =
        connect_postgres_admin("postgres_lists_and_revokes_device_sessions").await
    else {
        return;
    };
    let (database_name, database_url) = create_scratch_database(&admin_pool, &admin_url).await;
    let state = Arc::new(
        AppState::open_postgres(&database_url, &AppConfig::default())
            .await
            .unwrap(),
    );
    let server = start_server_with_state(state).await;
    let client = reqwest::Client::new();
    let credentials = serde_json::json!({
        "username": "postgres-session-user",
        "password": "test-password"
    });
    let first_token = client
        .post(format!("{server}/api/users/register"))
        .header(reqwest::header::USER_AGENT, "Chrome/140.0 Linux")
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
    let current_token = client
        .post(format!("{server}/api/users/login"))
        .header(reqwest::header::USER_AGENT, "Firefox/140.0 Windows")
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
        .get(format!("{server}/api/users/me/sessions"))
        .bearer_auth(&current_token)
        .send()
        .await
        .unwrap()
        .json::<Vec<serde_json::Value>>()
        .await
        .unwrap();
    assert_eq!(sessions.len(), 2);
    let other_id = sessions
        .iter()
        .find(|session| session["current"] == false)
        .unwrap()["id"]
        .as_str()
        .unwrap();
    assert_eq!(
        client
            .delete(format!("{server}/api/users/me/sessions/{other_id}"))
            .bearer_auth(&current_token)
            .send()
            .await
            .unwrap()
            .status(),
        204
    );
    assert_eq!(
        client
            .get(format!("{server}/api/users/me"))
            .bearer_auth(first_token)
            .send()
            .await
            .unwrap()
            .status(),
        401
    );

    server.shutdown().await;
    drop_scratch_database(&admin_pool, &database_name).await;
}
