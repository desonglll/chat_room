use std::{sync::Arc, time::Duration};

use chat_room::{build_app, state::AppState};
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio_tungstenite::{connect_async, tungstenite::Message};

mod support;
use support::session_token;

type Socket =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

async fn next_json(socket: &mut Socket) -> serde_json::Value {
    let frame = tokio::time::timeout(Duration::from_secs(3), socket.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let Message::Text(text) = frame else {
        panic!("expected text frame")
    };
    serde_json::from_str(&text).unwrap()
}

async fn next_type(socket: &mut Socket, kind: &str) -> serde_json::Value {
    loop {
        let message = next_json(socket).await;
        if message["type"] == kind {
            return message;
        }
    }
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
    assert_eq!(next_json(&mut socket).await["type"], "unread_counts");
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
    assert_eq!(next_json(&mut socket).await["type"], "auth_ok");
    socket
}

#[tokio::test]
async fn account_socket_pushes_cross_room_messages_but_never_self_messages() {
    let state = Arc::new(AppState::new().await.unwrap());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move { axum::serve(listener, build_app(state)).await.unwrap() });
    let alice = session_token(&base, "account-event-alice").await;
    let bob = session_token(&base, "account-event-bob").await;
    let room: serde_json::Value = reqwest::Client::new()
        .post(format!("{base}/api/rooms"))
        .bearer_auth(&alice)
        .json(&serde_json::json!({ "name": "background-room", "password": "" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let room_id = room["id"].as_str().unwrap();
    assert!(reqwest::Client::new()
        .post(format!("{base}/api/rooms/{room_id}/join-requests"))
        .bearer_auth(&bob)
        .json(&serde_json::json!({}))
        .send()
        .await
        .unwrap()
        .status()
        .is_success());

    let mut alice_account = account_socket(&base, &alice).await;
    let mut bob_account = account_socket(&base, &bob).await;
    let mut room = room_socket(&base, room_id, &alice).await;
    room.send(Message::Text(
        serde_json::json!({ "type": "message", "content": "background hello" }).to_string(),
    ))
    .await
    .unwrap();

    let event = next_type(&mut bob_account, "new_message").await;
    assert_eq!(event["room_id"], room_id);
    assert_eq!(event["room_name"], "background-room");
    assert_eq!(event["content"], "background hello");
    assert_eq!(event["sender"], "account-event-alice");

    let self_event = tokio::time::timeout(Duration::from_millis(1100), async {
        loop {
            if next_json(&mut alice_account).await["type"] == "new_message" {
                break;
            }
        }
    })
    .await;
    assert!(
        self_event.is_err(),
        "sender received its own account message event"
    );
    task.abort();
}
