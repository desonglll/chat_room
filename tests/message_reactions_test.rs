use std::sync::Arc;
use std::time::Duration;

use chat_room::{build_app, state::AppState};
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio_tungstenite::{connect_async, tungstenite::Message};

mod support;
use support::session_token;

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
    let task = tokio::spawn(async move { axum::serve(listener, build_app(state)).await.unwrap() });
    TestServer {
        base: format!("http://{address}"),
        task,
    }
}

async fn create_room(base: &str, token: &str) -> String {
    reqwest::Client::new()
        .post(format!("{base}/api/rooms"))
        .bearer_auth(token)
        .json(&serde_json::json!({ "name": "reactions", "password": "", "join_policy": "open" }))
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

async fn next_type(socket: &mut Socket, expected: &str) -> serde_json::Value {
    loop {
        let frame = tokio::time::timeout(Duration::from_secs(3), socket.next())
            .await
            .expect("timed out waiting for WebSocket frame")
            .expect("WebSocket ended")
            .expect("WebSocket error");
        let Message::Text(text) = frame else { continue };
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        if value["type"] == expected {
            return value;
        }
    }
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
    assert_eq!(next_type(&mut socket, "auth_ok").await["type"], "auth_ok");
    socket
}

async fn react(socket: &mut Socket, message_id: &str, emoji: &str, active: bool) {
    socket
        .send(Message::Text(
            serde_json::json!({
                "type": "reaction",
                "message_id": message_id,
                "emoji": emoji,
                "active": active,
            })
            .to_string(),
        ))
        .await
        .unwrap();
}

#[tokio::test]
async fn reactions_are_shared_idempotent_and_restored_with_history() {
    let server = start_server().await;
    let alice_token = session_token(&server.base, "reaction-alice").await;
    let bob_token = session_token(&server.base, "reaction-bob").await;
    let room_id = create_room(&server.base, &alice_token).await;
    let mut alice = open_room(&server.base, &room_id, &alice_token).await;
    let mut bob = open_room(&server.base, &room_id, &bob_token).await;

    alice
        .send(Message::Text(
            serde_json::json!({ "type": "message", "content": "react to me" }).to_string(),
        ))
        .await
        .unwrap();
    let sent = next_type(&mut alice, "broadcast").await;
    let message_id = sent["message_id"].as_str().unwrap();
    assert_eq!(
        next_type(&mut bob, "broadcast").await["message_id"],
        message_id
    );

    react(&mut alice, message_id, "👍", true).await;
    let alice_event = next_type(&mut alice, "reaction_changed").await;
    let alice_id = alice_event["user_id"].as_str().unwrap().to_string();
    assert_eq!(alice_event["active"], true);
    assert_eq!(next_type(&mut bob, "reaction_changed").await, alice_event);

    react(&mut alice, message_id, "👍", true).await;
    assert_eq!(
        next_type(&mut alice, "reaction_changed").await["active"],
        true
    );
    let _ = next_type(&mut bob, "reaction_changed").await;

    react(&mut bob, message_id, "👍", true).await;
    let bob_event = next_type(&mut alice, "reaction_changed").await;
    let bob_id = bob_event["user_id"].as_str().unwrap().to_string();
    let _ = next_type(&mut bob, "reaction_changed").await;

    let history: Vec<serde_json::Value> = reqwest::Client::new()
        .get(format!("{}/api/rooms/{room_id}/messages", server.base))
        .bearer_auth(&alice_token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let users = history[0]["reactions"][0]["user_ids"].as_array().unwrap();
    assert_eq!(users.len(), 2);
    assert!(users.iter().any(|id| id == &alice_id));
    assert!(users.iter().any(|id| id == &bob_id));

    drop(alice);
    let mut reconnected = open_room(&server.base, &room_id, &alice_token).await;
    let replayed = next_type(&mut reconnected, "broadcast").await;
    assert_eq!(replayed["message_id"], message_id);
    assert_eq!(
        replayed["reactions"][0]["user_ids"]
            .as_array()
            .unwrap()
            .len(),
        2
    );

    react(&mut reconnected, message_id, "👍", false).await;
    assert_eq!(
        next_type(&mut reconnected, "reaction_changed").await["active"],
        false
    );
    assert_eq!(
        next_type(&mut bob, "reaction_changed").await["active"],
        false
    );

    let history: Vec<serde_json::Value> = reqwest::Client::new()
        .get(format!("{}/api/rooms/{room_id}/messages", server.base))
        .bearer_auth(&alice_token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        history[0]["reactions"][0]["user_ids"],
        serde_json::json!([bob_id])
    );
}
