use std::sync::Arc;

use chat_room::{build_app, state::AppState};
use reqwest::header::USER_AGENT;
use tokio::net::TcpListener;
use tokio::time::Duration;

async fn start_server() -> (String, tokio::task::JoinHandle<()>) {
    let state = Arc::new(AppState::new().await.unwrap());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        axum::serve(listener, build_app(state)).await.unwrap();
    });
    (format!("http://{address}"), task)
}

async fn authenticate(
    client: &reqwest::Client,
    base: &str,
    endpoint: &str,
    user_agent: &str,
) -> String {
    let response = client
        .post(format!("{base}/api/users/{endpoint}"))
        .header(USER_AGENT, user_agent)
        .json(&serde_json::json!({
            "username": "session-user",
            "password": "test-password"
        }))
        .send()
        .await
        .unwrap();
    assert!(response.status().is_success());
    response.json::<serde_json::Value>().await.unwrap()["token"]
        .as_str()
        .unwrap()
        .to_owned()
}

async fn sessions(client: &reqwest::Client, base: &str, token: &str) -> Vec<serde_json::Value> {
    let response = client
        .get(format!("{base}/api/users/me/sessions"))
        .bearer_auth(token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    response.json().await.unwrap()
}

#[tokio::test]
async fn user_lists_privacy_safe_device_sessions() {
    let (base, task) = start_server().await;
    let client = reqwest::Client::new();
    let first_token = authenticate(
        &client,
        &base,
        "register",
        "Mozilla/5.0 (Macintosh) AppleWebKit/537.36 Chrome/140.0.0.0 Safari/537.36",
    )
    .await;
    let current_token = authenticate(
        &client,
        &base,
        "login",
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:140.0) Gecko/20100101 Firefox/140.0",
    )
    .await;
    tokio::time::sleep(Duration::from_millis(20)).await;
    assert_eq!(
        client
            .get(format!("{base}/api/users/me"))
            .bearer_auth(&current_token)
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    let sessions = sessions(&client, &base, &current_token).await;
    assert_eq!(sessions.len(), 2);

    let current = sessions
        .iter()
        .find(|session| session["current"] == true)
        .unwrap();
    assert_eq!(current["device_name"], "Firefox on Windows");
    assert!(current["ip_hint"].is_null() || current["ip_hint"] == "127.0.0.x");
    assert_ne!(current["ip_hint"], "127.0.0.1");
    assert!(current["created_at"].is_string());
    assert!(current["last_used_at"].is_string());
    assert!(current["last_used_at"].as_str() > current["created_at"].as_str());
    assert!(current["expires_at"].is_string());
    let management_ids = sessions
        .iter()
        .map(|session| session["id"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert!(management_ids.iter().all(|id| id.len() == 32));
    assert!(!management_ids.contains(&first_token.as_str()));
    assert!(!management_ids.contains(&current_token.as_str()));

    task.abort();
}

#[tokio::test]
async fn user_revokes_one_other_device_without_exposing_current_session() {
    let (base, task) = start_server().await;
    let client = reqwest::Client::new();
    let old_token = authenticate(&client, &base, "register", "Chrome/140.0 Linux").await;
    let current_token = authenticate(&client, &base, "login", "Firefox/140.0 Linux").await;
    let user_id = client
        .get(format!("{base}/api/users/me"))
        .bearer_auth(&current_token)
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let listed = sessions(&client, &base, &current_token).await;
    let old_id = listed
        .iter()
        .find(|session| session["current"] == false)
        .unwrap()["id"]
        .as_str()
        .unwrap();
    let current_id = listed
        .iter()
        .find(|session| session["current"] == true)
        .unwrap()["id"]
        .as_str()
        .unwrap();

    let revoke = client
        .delete(format!("{base}/api/users/me/sessions/{old_id}"))
        .bearer_auth(&current_token)
        .send()
        .await
        .unwrap();
    assert_eq!(revoke.status(), 204);
    assert_eq!(
        client
            .get(format!("{base}/api/users/me"))
            .bearer_auth(&old_token)
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    assert_eq!(
        client
            .get(format!("{base}/api/users/{user_id}"))
            .bearer_auth(&old_token)
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    assert_eq!(sessions(&client, &base, &current_token).await.len(), 1);

    let revoke_current = client
        .delete(format!("{base}/api/users/me/sessions/{current_id}"))
        .bearer_auth(&current_token)
        .send()
        .await
        .unwrap();
    assert_eq!(revoke_current.status(), 409);

    let foreign_token = {
        let response = client
            .post(format!("{base}/api/users/register"))
            .json(&serde_json::json!({
                "username": "foreign-session-user",
                "password": "test-password"
            }))
            .send()
            .await
            .unwrap();
        response.json::<serde_json::Value>().await.unwrap()["token"]
            .as_str()
            .unwrap()
            .to_owned()
    };
    let foreign_revoke = client
        .delete(format!("{base}/api/users/me/sessions/{current_id}"))
        .bearer_auth(foreign_token)
        .send()
        .await
        .unwrap();
    assert_eq!(foreign_revoke.status(), 404);

    task.abort();
}

#[tokio::test]
async fn user_revokes_all_other_devices_and_keeps_current_session() {
    let (base, task) = start_server().await;
    let client = reqwest::Client::new();
    let first_token = authenticate(&client, &base, "register", "Chrome/140.0 Linux").await;
    let current_token = authenticate(&client, &base, "login", "Firefox/140.0 Linux").await;
    let third_token = authenticate(&client, &base, "login", "Safari/18.0 Macintosh").await;

    let response = client
        .delete(format!("{base}/api/users/me/sessions/others"))
        .bearer_auth(&current_token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 204);

    for revoked in [&first_token, &third_token] {
        let response = client
            .get(format!("{base}/api/users/me"))
            .bearer_auth(revoked)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 401);
    }
    assert_eq!(
        client
            .get(format!("{base}/api/users/me"))
            .bearer_auth(&current_token)
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    let remaining = sessions(&client, &base, &current_token).await;
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0]["current"], true);

    let password_revoked_token = authenticate(&client, &base, "login", "Edge/140.0 Windows").await;
    assert_eq!(
        client
            .put(format!("{base}/api/users/me/password"))
            .bearer_auth(&current_token)
            .json(&serde_json::json!({
                "current_password": "test-password",
                "new_password": "updated-password"
            }))
            .send()
            .await
            .unwrap()
            .status(),
        204
    );
    assert_eq!(
        client
            .get(format!("{base}/api/users/me"))
            .bearer_auth(password_revoked_token)
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    assert_eq!(
        client
            .get(format!("{base}/api/users/me"))
            .bearer_auth(&current_token)
            .send()
            .await
            .unwrap()
            .status(),
        200
    );

    task.abort();
}
