use std::sync::Arc;

use chat_room::{build_app, state::AppState};
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use uuid::Uuid;

mod support;
use support::session_token;

#[tokio::test]
async fn join_request_accepts_legacy_non_utf8_role_ids() {
    let state = Arc::new(AppState::new().await.unwrap());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn({
        let state = state.clone();
        async move { axum::serve(listener, build_app(state)).await.unwrap() }
    });
    let owner_token = session_token(&base, "legacy-role-owner").await;
    let room: serde_json::Value = reqwest::Client::new()
        .post(format!("{base}/api/rooms"))
        .bearer_auth(owner_token)
        .json(&serde_json::json!({
            "name": "legacy-role-room",
            "password": "secret",
            "join_policy": "approval"
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let room_id = Uuid::parse_str(room["id"].as_str().unwrap()).unwrap();

    let current_id: String =
        sqlx::query_scalar("SELECT id FROM room_roles WHERE room_id = ? AND name = 'member'")
            .bind(room_id)
            .fetch_one(state.pool())
            .await
            .unwrap();
    let mut legacy_id = room_id.as_bytes().to_vec();
    legacy_id.extend_from_slice(b":member");
    let mut transaction = state.pool().begin().await.unwrap();
    sqlx::query("PRAGMA defer_foreign_keys = ON")
        .execute(&mut *transaction)
        .await
        .unwrap();
    sqlx::query("UPDATE room_role_permissions SET role_id = CAST(? AS TEXT) WHERE role_id = ?")
        .bind(&legacy_id)
        .bind(&current_id)
        .execute(&mut *transaction)
        .await
        .unwrap();
    sqlx::query("UPDATE room_roles SET id = CAST(? AS TEXT) WHERE id = ?")
        .bind(&legacy_id)
        .bind(&current_id)
        .execute(&mut *transaction)
        .await
        .unwrap();
    transaction.commit().await.unwrap();

    let member_token = session_token(&base, "legacy-role-member").await;
    let response = reqwest::Client::new()
        .post(format!("{base}/api/rooms/{room_id}/join-requests"))
        .bearer_auth(member_token)
        .json(&serde_json::json!({ "password": "secret" }))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 202, "{}", response.text().await.unwrap());
    task.abort();
}

#[tokio::test]
async fn room_lookup_does_not_inherit_the_creators_membership() {
    let state = Arc::new(AppState::new().await.unwrap());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn({
        let state = state.clone();
        async move { axum::serve(listener, build_app(state)).await.unwrap() }
    });
    let owner_token = session_token(&base, "lookup-owner").await;
    let room: serde_json::Value = reqwest::Client::new()
        .post(format!("{base}/api/rooms"))
        .bearer_auth(owner_token)
        .json(&serde_json::json!({
            "name": "lookup-membership-room",
            "password": "room-secret",
            "join_policy": "open"
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let viewer_token = session_token(&base, "lookup-viewer").await;

    let lookup: serde_json::Value = reqwest::Client::new()
        .get(format!("{base}/api/rooms/{}", room["id"].as_str().unwrap()))
        .bearer_auth(viewer_token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    assert!(lookup["membership_status"].is_null());
    assert!(lookup["membership_role"].is_null());
    task.abort();
}

#[tokio::test]
async fn active_member_reconnects_to_password_room_and_sends_without_password() {
    let state = Arc::new(AppState::new().await.unwrap());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn({
        let state = state.clone();
        async move { axum::serve(listener, build_app(state)).await.unwrap() }
    });
    let owner_token = session_token(&base, "password-room-owner").await;
    let room: serde_json::Value = reqwest::Client::new()
        .post(format!("{base}/api/rooms"))
        .bearer_auth(&owner_token)
        .json(&serde_json::json!({
            "name": "password-room-reconnect",
            "password": "secret",
            "join_policy": "approval"
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let room_id = room["id"].as_str().unwrap();
    let websocket_base = base.replacen("http://", "ws://", 1);
    let outsider_token = session_token(&base, "password-room-outsider").await;
    let (mut outsider, _) = connect_async(format!("{websocket_base}/ws/{room_id}"))
        .await
        .unwrap();
    outsider
        .send(Message::Text(
            serde_json::json!({ "type": "join", "token": outsider_token }).to_string(),
        ))
        .await
        .unwrap();
    assert_eq!(next_socket_json(&mut outsider).await["type"], "auth_fail");

    let (mut socket, _) = connect_async(format!("{websocket_base}/ws/{room_id}"))
        .await
        .unwrap();
    socket
        .send(Message::Text(
            serde_json::json!({ "type": "join", "token": owner_token }).to_string(),
        ))
        .await
        .unwrap();

    let auth = next_socket_json(&mut socket).await;
    assert_eq!(auth["type"], "auth_ok");
    socket
        .send(Message::Text(
            serde_json::json!({ "type": "message", "content": "sent after reconnect" }).to_string(),
        ))
        .await
        .unwrap();
    loop {
        let event = next_socket_json(&mut socket).await;
        if event["type"] == "broadcast" {
            assert_eq!(event["content"], "sent after reconnect");
            break;
        }
    }
    task.abort();
}

async fn next_socket_json(
    socket: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
) -> serde_json::Value {
    let frame = tokio::time::timeout(std::time::Duration::from_secs(2), socket.next())
        .await
        .expect("timed out waiting for WebSocket frame")
        .expect("WebSocket ended")
        .expect("WebSocket error");
    let Message::Text(text) = frame else {
        panic!("expected text WebSocket frame");
    };
    serde_json::from_str(&text).unwrap()
}
