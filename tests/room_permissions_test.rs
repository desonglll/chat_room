use std::{sync::Arc, time::Duration};

use chat_room::{build_app, state::AppState};
use futures_util::{SinkExt, StreamExt};
use reqwest::StatusCode;
use tokio::net::TcpListener;
use tokio_tungstenite::{connect_async, tungstenite::Message};

type Socket =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

struct TestServer {
    base: String,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn start_server() -> TestServer {
    let state = Arc::new(AppState::new().await.unwrap());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        axum::serve(listener, build_app(state)).await.unwrap();
    });
    TestServer {
        base: format!("http://{address}"),
        task,
    }
}

async fn account(base: &str, username: &str) -> (String, String) {
    let response: serde_json::Value = reqwest::Client::new()
        .post(format!("{base}/api/users/register"))
        .json(&serde_json::json!({
            "username": username,
            "password": "test-password"
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    (
        response["token"].as_str().unwrap().to_string(),
        response["user"]["id"].as_str().unwrap().to_string(),
    )
}

async fn create_room(base: &str, token: &str, name: &str, policy: &str) -> String {
    reqwest::Client::new()
        .post(format!("{base}/api/rooms"))
        .bearer_auth(token)
        .json(&serde_json::json!({
            "name": name,
            "password": "",
            "join_policy": policy
        }))
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string()
}

async fn open_room(base: &str, room_id: &str, token: &str) -> Socket {
    let url = format!("{}/ws/{room_id}", base.replacen("http://", "ws://", 1));
    let (mut socket, _) = connect_async(url).await.unwrap();
    socket
        .send(Message::Text(
            serde_json::json!({ "type": "join", "token": token }).to_string(),
        ))
        .await
        .unwrap();
    socket
}

async fn next_json(socket: &mut Socket) -> serde_json::Value {
    loop {
        let frame = tokio::time::timeout(Duration::from_secs(4), socket.next())
            .await
            .expect("timed out waiting for WebSocket frame")
            .expect("WebSocket ended")
            .expect("WebSocket error");
        let Message::Text(text) = frame else { continue };
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        if value["type"] != "history_complete" {
            return value;
        }
    }
}

async fn next_type(socket: &mut Socket, expected: &str) -> serde_json::Value {
    loop {
        let value = next_json(socket).await;
        if value["type"] == expected {
            return value;
        }
    }
}

#[tokio::test]
async fn owner_controls_requests_invitations_roles_and_active_roster() {
    let server = start_server().await;
    let client = reqwest::Client::new();
    let (owner_token, _) = account(&server.base, "permissions-owner").await;
    let (applicant_token, applicant_id) = account(&server.base, "permissions-applicant").await;
    let (invitee_token, invitee_id) = account(&server.base, "permissions-invitee").await;
    let (_, outsider_id) = account(&server.base, "permissions-outsider").await;
    let room_id = create_room(&server.base, &owner_token, "approval-room", "approval").await;
    let join_url = format!("{}/api/rooms/{room_id}/join-requests", server.base);
    let members_url = format!("{}/api/rooms/{room_id}/members", server.base);

    let pending = client
        .post(&join_url)
        .bearer_auth(&applicant_token)
        .json(&serde_json::json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(pending.status(), StatusCode::ACCEPTED);
    assert_eq!(
        pending.json::<serde_json::Value>().await.unwrap()["status"],
        "pending"
    );

    let mut pending_socket = open_room(&server.base, &room_id, &applicant_token).await;
    assert_eq!(next_json(&mut pending_socket).await["type"], "auth_fail");

    assert_eq!(
        client
            .get(&members_url)
            .bearer_auth(&applicant_token)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    let memberships: Vec<serde_json::Value> = client
        .get(&members_url)
        .bearer_auth(&owner_token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(memberships
        .iter()
        .any(|item| item["user_id"] == applicant_id));
    assert!(!memberships
        .iter()
        .any(|item| item["user_id"] == outsider_id));

    let applicant_url = format!("{members_url}/{applicant_id}");
    let approved = client
        .patch(&applicant_url)
        .bearer_auth(&owner_token)
        .json(&serde_json::json!({ "action": "approve" }))
        .send()
        .await
        .unwrap();
    assert_eq!(approved.status(), StatusCode::OK);
    assert_eq!(
        approved.json::<serde_json::Value>().await.unwrap()["status"],
        "active"
    );

    let invited = client
        .post(format!("{}/api/rooms/{room_id}/invitations", server.base))
        .bearer_auth(&owner_token)
        .json(&serde_json::json!({ "username": "permissions-invitee" }))
        .send()
        .await
        .unwrap();
    assert_eq!(invited.status(), StatusCode::OK);
    assert_eq!(
        invited.json::<serde_json::Value>().await.unwrap()["status"],
        "invited"
    );
    let accepted = client
        .post(&join_url)
        .bearer_auth(&invitee_token)
        .json(&serde_json::json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(accepted.status(), StatusCode::OK);

    let promoted = client
        .patch(&applicant_url)
        .bearer_auth(&owner_token)
        .json(&serde_json::json!({ "action": "set_role", "role": "admin" }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        promoted.json::<serde_json::Value>().await.unwrap()["role"],
        "admin"
    );
    assert_eq!(
        client
            .patch(format!("{}/api/rooms/{room_id}", server.base))
            .bearer_auth(&applicant_token)
            .json(&serde_json::json!({ "name": "admin-renamed-room" }))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        client
            .patch(format!("{}/api/rooms/{room_id}", server.base))
            .bearer_auth(&invitee_token)
            .json(&serde_json::json!({ "name": "forbidden-name" }))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );

    assert_eq!(
        client
            .delete(format!("{members_url}/me"))
            .bearer_auth(&invitee_token)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::NO_CONTENT
    );
    let removed = client
        .patch(&applicant_url)
        .bearer_auth(&owner_token)
        .json(&serde_json::json!({ "action": "remove" }))
        .send()
        .await
        .unwrap();
    assert_eq!(removed.status(), StatusCode::OK);
    let memberships: Vec<serde_json::Value> = client
        .get(&members_url)
        .bearer_auth(&owner_token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(!memberships
        .iter()
        .any(|item| item["user_id"] == applicant_id));
    assert!(!memberships.iter().any(|item| item["user_id"] == invitee_id));
    assert_eq!(
        client
            .delete(format!("{members_url}/me"))
            .bearer_auth(owner_token)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::CONFLICT
    );
}

#[tokio::test]
async fn account_socket_updates_unread_count_after_message_and_read() {
    let server = start_server().await;
    let (owner_token, _) = account(&server.base, "unread-owner").await;
    let (reader_token, _) = account(&server.base, "unread-reader").await;
    let room_id = create_room(&server.base, &owner_token, "unread-room", "open").await;
    let mut owner = open_room(&server.base, &room_id, &owner_token).await;
    assert_eq!(next_json(&mut owner).await["type"], "auth_ok");
    let _ = next_type(&mut owner, "presence").await;
    let mut reader = open_room(&server.base, &room_id, &reader_token).await;
    assert_eq!(next_json(&mut reader).await["type"], "auth_ok");
    let _ = next_type(&mut reader, "system").await;
    let _ = next_type(&mut owner, "system").await;

    let account_url = format!("{}/ws/account", server.base.replacen("http://", "ws://", 1));
    let (mut account_socket, _) = connect_async(account_url).await.unwrap();
    account_socket
        .send(Message::Text(
            serde_json::json!({ "token": reader_token }).to_string(),
        ))
        .await
        .unwrap();
    let initial = next_type(&mut account_socket, "unread_counts").await;
    assert_eq!(initial["rooms"][0]["unread_count"], 0);

    owner
        .send(Message::Text(
            serde_json::json!({ "type": "message", "content": "new unread" }).to_string(),
        ))
        .await
        .unwrap();
    let incoming = next_type(&mut reader, "broadcast").await;
    let unread = next_type(&mut account_socket, "unread_counts").await;
    assert_eq!(unread["rooms"][0]["unread_count"], 1);

    reader
        .send(Message::Text(
            serde_json::json!({ "type": "read", "message_id": incoming["message_id"] }).to_string(),
        ))
        .await
        .unwrap();
    let read = next_type(&mut account_socket, "unread_counts").await;
    assert_eq!(read["rooms"][0]["unread_count"], 0);
}

#[tokio::test]
async fn account_socket_surfaces_join_requests_to_room_managers() {
    let server = start_server().await;
    let client = reqwest::Client::new();
    let (owner_token, _) = account(&server.base, "request-notice-owner").await;
    let (applicant_token, applicant_id) = account(&server.base, "request-notice-applicant").await;
    let room_id = create_room(
        &server.base,
        &owner_token,
        "request-notice-room",
        "approval",
    )
    .await;
    let account_url = format!("{}/ws/account", server.base.replacen("http://", "ws://", 1));
    let (mut owner_account, _) = connect_async(account_url).await.unwrap();
    owner_account
        .send(Message::Text(
            serde_json::json!({ "token": owner_token }).to_string(),
        ))
        .await
        .unwrap();
    let initial = next_type(&mut owner_account, "unread_counts").await;
    let room = initial["rooms"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["room_id"] == room_id)
        .unwrap();
    assert_eq!(room["pending_join_requests"], 0);

    assert_eq!(
        client
            .post(format!("{}/api/rooms/{room_id}/join-requests", server.base))
            .bearer_auth(&applicant_token)
            .json(&serde_json::json!({}))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::ACCEPTED
    );

    let updated = next_type(&mut owner_account, "unread_counts").await;
    let room = updated["rooms"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["room_id"] == room_id)
        .unwrap();
    assert_eq!(room["pending_join_requests"], 1);
    assert!(room["pending_join_requested_at"].is_string());

    let conversations: Vec<serde_json::Value> = client
        .get(format!("{}/api/conversations", server.base))
        .bearer_auth(&owner_token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let conversation = conversations
        .iter()
        .find(|item| item["room_id"] == room_id)
        .unwrap();
    assert_eq!(conversation["pending_join_requests"], 1);

    let applicant_conversations: Vec<serde_json::Value> = client
        .get(format!("{}/api/conversations", server.base))
        .bearer_auth(&applicant_token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(applicant_conversations[0]["pending_join_requests"], 0);

    assert_eq!(
        client
            .patch(format!(
                "{}/api/rooms/{room_id}/members/{applicant_id}",
                server.base
            ))
            .bearer_auth(&owner_token)
            .json(&serde_json::json!({ "action": "approve" }))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    let resolved = next_type(&mut owner_account, "unread_counts").await;
    let room = resolved["rooms"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["room_id"] == room_id)
        .unwrap();
    assert_eq!(room["pending_join_requests"], 0);
}
