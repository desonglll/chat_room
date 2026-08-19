use std::{sync::Arc, time::Duration};

use chat_room::{build_app, state::AppState};
use futures_util::{SinkExt, StreamExt};
use reqwest::Client;
use tokio::net::TcpListener;
use tokio_tungstenite::{connect_async, tungstenite::Message};

type Socket =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

struct Account {
    id: String,
    token: String,
}

async fn register(client: &Client, base: &str, username: &str) -> Account {
    let value = client
        .post(format!("{base}/api/users/register"))
        .json(&serde_json::json!({ "username": username, "password": "test-password" }))
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();
    Account {
        id: value["user"]["id"].as_str().unwrap().to_string(),
        token: value["token"].as_str().unwrap().to_string(),
    }
}

async fn next_type(socket: &mut Socket, expected: &str) -> serde_json::Value {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let frame = socket.next().await.unwrap().unwrap();
            let Message::Text(text) = frame else { continue };
            let value: serde_json::Value = serde_json::from_str(&text).unwrap();
            if value["type"] == expected {
                return value;
            }
        }
    })
    .await
    .unwrap_or_else(|_| panic!("timed out waiting for account event: {expected}"))
}

async fn next_disconnect(socket: &mut Socket) -> serde_json::Value {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let frame = socket.next().await.unwrap().unwrap();
            let Message::Text(text) = frame else { continue };
            let value: serde_json::Value = serde_json::from_str(&text).unwrap();
            if matches!(
                value["content"].as_str(),
                Some("friendship removed" | "membership left")
            ) {
                return value;
            }
        }
    })
    .await
    .expect("timed out waiting for room event")
}

async fn account_socket(base: &str, token: &str) -> Socket {
    let (mut socket, _) = connect_async(format!(
        "{}/ws/account",
        base.replacen("http://", "ws://", 1)
    ))
    .await
    .unwrap();
    socket
        .send(Message::Text(
            serde_json::json!({ "token": token }).to_string(),
        ))
        .await
        .unwrap();
    socket
}

async fn room_socket(base: &str, room_id: &str, token: &str) -> Socket {
    let (mut socket, _) = connect_async(format!(
        "{}/ws/{room_id}",
        base.replacen("http://", "ws://", 1)
    ))
    .await
    .unwrap();
    socket
        .send(Message::Text(
            serde_json::json!({ "type": "join", "token": token }).to_string(),
        ))
        .await
        .unwrap();
    assert_eq!(next_type(&mut socket, "auth_ok").await["type"], "auth_ok");
    socket
}

#[tokio::test]
async fn account_socket_reports_friend_changes_and_direct_message_titles() {
    let state = Arc::new(AppState::new().await.unwrap());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        axum::serve(listener, build_app(state)).await.unwrap();
    });
    let client = Client::new();
    let alice = register(&client, &base, "realtime-alice").await;
    let bob = register(&client, &base, "realtime-bob").await;
    let mut bob_account = account_socket(&base, &bob.token).await;

    assert!(next_type(&mut bob_account, "unread_counts").await["rooms"]
        .as_array()
        .unwrap()
        .is_empty());
    assert_eq!(
        next_type(&mut bob_account, "social_changed").await["incoming_request_count"],
        0
    );

    client
        .post(format!("{base}/api/friend-requests"))
        .bearer_auth(&alice.token)
        .json(&serde_json::json!({ "user_id": bob.id }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        next_type(&mut bob_account, "social_changed").await["incoming_request_count"],
        1
    );
    client
        .patch(format!("{base}/api/friend-requests/{}", alice.id))
        .bearer_auth(&bob.token)
        .json(&serde_json::json!({ "action": "accept" }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        next_type(&mut bob_account, "social_changed").await["incoming_request_count"],
        0
    );

    let conversation = client
        .post(format!("{base}/api/direct-chats"))
        .bearer_auth(&alice.token)
        .json(&serde_json::json!({ "user_id": bob.id }))
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();
    let room_id = conversation["room_id"].as_str().unwrap();
    let mut alice_room = room_socket(&base, room_id, &alice.token).await;
    alice_room
        .send(Message::Text(
            serde_json::json!({ "type": "message", "content": "account direct" }).to_string(),
        ))
        .await
        .unwrap();
    next_type(&mut alice_room, "broadcast").await;

    let event = next_type(&mut bob_account, "new_message").await;
    assert_eq!(event["conversation_kind"], "direct");
    assert_eq!(event["conversation_title"], "realtime-alice");
    assert_eq!(event["room_id"], room_id);
    assert_eq!(event["content"], "account direct");

    let mut bob_room = room_socket(&base, room_id, &bob.token).await;
    assert_eq!(
        client
            .delete(format!("{base}/api/friends/{}", bob.id))
            .bearer_auth(&alice.token)
            .send()
            .await
            .unwrap()
            .status(),
        reqwest::StatusCode::NO_CONTENT
    );
    assert_eq!(next_disconnect(&mut alice_room).await["type"], "system");
    assert_eq!(next_disconnect(&mut bob_room).await["type"], "system");
    task.abort();
}
