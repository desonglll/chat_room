use std::sync::Arc;

use chat_room::{build_app, state::AppState};
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use uuid::Uuid;

async fn start_server() -> (String, Arc<AppState>, tokio::task::JoinHandle<()>) {
    let state = Arc::new(AppState::new().await.unwrap());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let app = build_app(state.clone());
    let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (format!("http://{address}"), state, task)
}

#[tokio::test]
async fn registration_login_and_logout_manage_uuid_sessions() {
    let (base, state, task) = start_server().await;
    let client = reqwest::Client::new();
    let credentials = serde_json::json!({
        "username": "Alice",
        "password": "correct-horse"
    });

    let invalid = client
        .post(format!("{base}/api/users/register"))
        .json(&serde_json::json!({ "username": "Alice", "password": "short" }))
        .send()
        .await
        .unwrap();
    assert_eq!(invalid.status(), 400);

    let response = client
        .post(format!("{base}/api/users/register"))
        .json(&credentials)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 201);
    let session: serde_json::Value = response.json().await.unwrap();
    let token = session["token"].as_str().unwrap().to_string();
    let user_id = session["user"]["id"].as_str().unwrap().to_string();
    assert!(Uuid::parse_str(&token).is_ok());
    assert!(Uuid::parse_str(&user_id).is_ok());
    assert_eq!(session["user"]["username"], "Alice");

    let stored_hash: String =
        sqlx::query_scalar("SELECT password_hash FROM users WHERE username = ?")
            .bind("Alice")
            .fetch_one(state.pool())
            .await
            .unwrap();
    assert!(stored_hash.starts_with("$argon2"));
    assert!(!stored_hash.contains("correct-horse"));

    let duplicate = client
        .post(format!("{base}/api/users/register"))
        .json(&serde_json::json!({ "username": "alice", "password": "another-password" }))
        .send()
        .await
        .unwrap();
    assert_eq!(duplicate.status(), 409);

    let wrong = client
        .post(format!("{base}/api/users/login"))
        .json(&serde_json::json!({ "username": "Alice", "password": "wrong-password" }))
        .send()
        .await
        .unwrap();
    assert_eq!(wrong.status(), 401);

    let login = client
        .post(format!("{base}/api/users/login"))
        .json(&credentials)
        .send()
        .await
        .unwrap();
    assert_eq!(login.status(), 200);
    let login_session: serde_json::Value = login.json().await.unwrap();
    let login_token = login_session["token"].as_str().unwrap();
    assert_ne!(login_token, token);

    let me = client
        .get(format!("{base}/api/users/me"))
        .bearer_auth(login_token)
        .send()
        .await
        .unwrap();
    assert_eq!(me.status(), 200);
    assert_eq!(me.json::<serde_json::Value>().await.unwrap()["id"], user_id);

    let room: serde_json::Value = client
        .post(format!("{base}/api/rooms"))
        .json(&serde_json::json!({ "name": "identity-test", "password": "" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let room_id = room["id"].as_str().unwrap();
    let ws_url = format!("{}/ws/{room_id}", base.replace("http://", "ws://"));

    let (mut legacy_socket, _) = connect_async(&ws_url).await.unwrap();
    legacy_socket
        .send(Message::Text(
            serde_json::json!({ "type": "join", "username": "impostor" }).to_string(),
        ))
        .await
        .unwrap();
    let legacy_response = legacy_socket.next().await.unwrap().unwrap();
    let Message::Text(legacy_text) = legacy_response else {
        panic!("expected legacy authentication rejection");
    };
    let legacy_json: serde_json::Value = serde_json::from_str(&legacy_text).unwrap();
    assert_eq!(legacy_json["type"], "auth_fail");

    let (mut socket, _) = connect_async(&ws_url).await.unwrap();
    socket
        .send(Message::Text(
            serde_json::json!({ "type": "join", "token": login_token }).to_string(),
        ))
        .await
        .unwrap();
    assert_eq!(next_json(&mut socket).await["type"], "auth_ok");
    assert_eq!(next_json(&mut socket).await["type"], "system");
    socket
        .send(Message::Text(
            serde_json::json!({ "type": "message", "content": "account-bound" }).to_string(),
        ))
        .await
        .unwrap();
    let broadcast = next_json(&mut socket).await;
    assert_eq!(broadcast["sender"], "Alice");
    assert_eq!(broadcast["sender_id"], user_id);
    assert!(Uuid::parse_str(broadcast["message_id"].as_str().unwrap()).is_ok());

    let logout = client
        .post(format!("{base}/api/users/logout"))
        .bearer_auth(login_token)
        .send()
        .await
        .unwrap();
    assert_eq!(logout.status(), 204);
    assert_eq!(
        client
            .get(format!("{base}/api/users/me"))
            .bearer_auth(login_token)
            .send()
            .await
            .unwrap()
            .status(),
        401
    );

    task.abort();
}

async fn next_json(
    socket: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
) -> serde_json::Value {
    let frame = socket.next().await.unwrap().unwrap();
    let Message::Text(text) = frame else {
        panic!("expected text WebSocket frame");
    };
    serde_json::from_str(&text).unwrap()
}
