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
    state: Arc<AppState>,
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
    let app = build_app(state.clone());
    let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    TestServer {
        base: format!("http://{address}"),
        state,
        task,
    }
}

async fn create_room(base: &str, name: &str, owner: &str) -> String {
    let owner_token = session_token(base, owner).await;
    reqwest::Client::new()
        .post(format!("{base}/api/rooms"))
        .bearer_auth(owner_token)
        .json(&serde_json::json!({ "name": name, "password": "", "join_policy": "open" }))
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

async fn open_room(base: &str, room_id: &str, token: &str) -> (Socket, serde_json::Value) {
    let url = format!("{}/ws/{room_id}", base.replacen("http://", "ws://", 1));
    let (mut socket, _) = connect_async(url).await.unwrap();
    socket
        .send(Message::Text(
            serde_json::json!({ "type": "join", "token": token }).to_string(),
        ))
        .await
        .unwrap();
    let auth = next_json(&mut socket).await;
    assert_eq!(auth["type"], "auth_ok");
    (socket, auth)
}

async fn next_json(socket: &mut Socket) -> serde_json::Value {
    let frame = tokio::time::timeout(Duration::from_secs(3), socket.next())
        .await
        .expect("timed out waiting for WebSocket frame")
        .expect("WebSocket ended")
        .expect("WebSocket error");
    let Message::Text(text) = frame else {
        panic!("expected text frame, got {frame:?}");
    };
    serde_json::from_str(&text).unwrap()
}

async fn next_json_type(socket: &mut Socket, expected: &str) -> serde_json::Value {
    loop {
        let value = next_json(socket).await;
        if value["type"] == expected {
            return value;
        }
    }
}

async fn send_message(socket: &mut Socket, content: &str, reply_to: Option<&str>) {
    socket
        .send(Message::Text(
            serde_json::json!({
                "type": "message",
                "content": content,
                "reply_to": reply_to,
            })
            .to_string(),
        ))
        .await
        .unwrap();
}

#[tokio::test]
async fn replies_are_broadcast_and_replayed_with_a_stable_preview() {
    let server = start_server().await;
    let room_id = create_room(&server.base, "replies", "reply-alice").await;
    let alice_token = session_token(&server.base, "reply-alice").await;
    let bob_token = session_token(&server.base, "reply-bob").await;
    let (mut alice, alice_auth) = open_room(&server.base, &room_id, &alice_token).await;
    assert_eq!(alice_auth["members"].as_array().unwrap().len(), 1);
    assert_eq!(next_json(&mut alice).await["type"], "presence");

    let (mut bob, bob_auth) = open_room(&server.base, &room_id, &bob_token).await;
    assert_eq!(bob_auth["members"].as_array().unwrap().len(), 2);
    assert_eq!(next_json(&mut bob).await["type"], "system");
    assert_eq!(
        next_json(&mut alice).await["content"],
        "reply-bob joined the room"
    );

    send_message(&mut alice, "original message", None).await;
    let original_for_alice = next_json_type(&mut alice, "broadcast").await;
    let original_for_bob = next_json_type(&mut bob, "broadcast").await;
    assert_eq!(original_for_bob["content"], "original message");
    let original_id = original_for_alice["message_id"].as_str().unwrap();

    send_message(&mut bob, "quoted response", Some(original_id)).await;
    let reply = next_json_type(&mut alice, "broadcast").await;
    assert_eq!(reply["content"], "quoted response");
    assert_eq!(reply["reply_to"]["message_id"], original_id);
    assert_eq!(reply["reply_to"]["sender"], "reply-alice");
    assert_eq!(reply["reply_to"]["content"], "original message");
    assert_eq!(
        next_json_type(&mut bob, "broadcast").await["reply_to"],
        reply["reply_to"]
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
    assert_eq!(history.len(), 2);
    assert_eq!(history[1]["reply_to"], reply["reply_to"]);
}

#[tokio::test]
async fn recall_is_sender_only_and_redacts_without_deleting_the_record() {
    let server = start_server().await;
    let room_id = create_room(&server.base, "recall", "recall-alice").await;
    let alice_token = session_token(&server.base, "recall-alice").await;
    let bob_token = session_token(&server.base, "recall-bob").await;
    let (mut alice, _) = open_room(&server.base, &room_id, &alice_token).await;
    assert_eq!(next_json(&mut alice).await["type"], "presence");
    let (mut bob, _) = open_room(&server.base, &room_id, &bob_token).await;
    assert_eq!(next_json(&mut bob).await["type"], "system");
    assert_eq!(next_json(&mut alice).await["type"], "system");

    send_message(&mut alice, "retained original", None).await;
    let original = next_json_type(&mut alice, "broadcast").await;
    assert_eq!(
        next_json_type(&mut bob, "broadcast").await["content"],
        "retained original"
    );
    let message_id = original["message_id"].as_str().unwrap();
    let message_uuid = uuid::Uuid::parse_str(message_id).unwrap();

    bob.send(Message::Text(
        serde_json::json!({ "type": "recall", "message_id": message_id }).to_string(),
    ))
    .await
    .unwrap();
    tokio::time::sleep(Duration::from_millis(80)).await;
    let unauthorized_recall: Option<String> =
        sqlx::query_scalar("SELECT recalled_at FROM messages WHERE id = ?")
            .bind(message_uuid)
            .fetch_one(server.state.pool())
            .await
            .unwrap();
    assert!(unauthorized_recall.is_none());

    alice
        .send(Message::Text(
            serde_json::json!({ "type": "recall", "message_id": message_id }).to_string(),
        ))
        .await
        .unwrap();
    assert_eq!(
        next_json_type(&mut alice, "message_recalled").await["type"],
        "message_recalled"
    );
    assert_eq!(
        next_json_type(&mut bob, "message_recalled").await["message_id"],
        message_id
    );

    let stored: (String, Option<String>) =
        sqlx::query_as("SELECT content, recalled_at FROM messages WHERE id = ?")
            .bind(message_uuid)
            .fetch_one(server.state.pool())
            .await
            .unwrap();
    assert_eq!(stored.0, "retained original");
    assert!(stored.1.is_some());

    let history: Vec<serde_json::Value> = reqwest::Client::new()
        .get(format!("{}/api/rooms/{room_id}/messages", server.base))
        .bearer_auth(&alice_token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0]["content"], "");
    assert!(history[0]["recalled_at"].is_string());
}

#[tokio::test]
async fn presence_lists_unique_accounts_and_updates_when_the_last_tab_leaves() {
    let server = start_server().await;
    let room_id = create_room(&server.base, "presence", "presence-alice").await;
    let alice_token = session_token(&server.base, "presence-alice").await;
    let bob_token = session_token(&server.base, "presence-bob").await;

    let (mut alice_first, first_auth) = open_room(&server.base, &room_id, &alice_token).await;
    assert_eq!(first_auth["members"].as_array().unwrap().len(), 1);
    assert_eq!(next_json(&mut alice_first).await["type"], "presence");

    let (mut alice_second, second_auth) = open_room(&server.base, &room_id, &alice_token).await;
    assert_eq!(second_auth["members"].as_array().unwrap().len(), 1);

    let (bob, bob_auth) = open_room(&server.base, &room_id, &bob_token).await;
    assert_eq!(bob_auth["members"].as_array().unwrap().len(), 2);
    let joined_first = next_json(&mut alice_first).await;
    let joined_second = next_json(&mut alice_second).await;
    assert_eq!(joined_first["members"].as_array().unwrap().len(), 2);
    assert_eq!(joined_second["members"].as_array().unwrap().len(), 2);

    drop(bob);
    let left_first = next_json_type(&mut alice_first, "presence").await;
    let left_second = next_json_type(&mut alice_second, "presence").await;
    assert_eq!(left_first["members"].as_array().unwrap().len(), 1);
    assert_eq!(left_second["members"].as_array().unwrap().len(), 1);

    let (mut bob_reconnected, _) = open_room(&server.base, &room_id, &bob_token).await;
    assert_eq!(next_json(&mut bob_reconnected).await["type"], "presence");
    assert_eq!(next_json(&mut alice_first).await["type"], "presence");
    assert_eq!(next_json(&mut alice_second).await["type"], "presence");
    drop(bob_reconnected);
    let _ = next_json_type(&mut alice_first, "presence").await;
    let _ = next_json_type(&mut alice_second, "presence").await;

    drop(alice_second);
    let unexpected = tokio::time::timeout(Duration::from_millis(300), alice_first.next()).await;
    assert!(
        unexpected.is_err(),
        "closing one duplicate tab must not emit a leave event"
    );
}

#[tokio::test]
async fn read_receipts_advance_monotonically_and_restore_on_reconnect() {
    let server = start_server().await;
    let room_id = create_room(&server.base, "read-receipts", "read-alice").await;
    let alice_token = session_token(&server.base, "read-alice").await;
    let bob_token = session_token(&server.base, "read-bob").await;
    let (mut alice, _) = open_room(&server.base, &room_id, &alice_token).await;
    assert_eq!(next_json(&mut alice).await["type"], "presence");
    let (mut bob, _) = open_room(&server.base, &room_id, &bob_token).await;
    assert_eq!(next_json(&mut bob).await["type"], "system");
    assert_eq!(next_json(&mut alice).await["type"], "system");

    send_message(&mut alice, "first", None).await;
    let first = next_json_type(&mut alice, "broadcast").await;
    assert_eq!(
        next_json_type(&mut bob, "broadcast").await["content"],
        "first"
    );
    send_message(&mut alice, "second", None).await;
    let second = next_json_type(&mut alice, "broadcast").await;
    assert_eq!(
        next_json_type(&mut bob, "broadcast").await["content"],
        "second"
    );

    let second_id = second["message_id"].as_str().unwrap();
    bob.send(Message::Text(
        serde_json::json!({ "type": "read", "message_id": second_id }).to_string(),
    ))
    .await
    .unwrap();
    let receipt = next_json_type(&mut alice, "read_receipt").await;
    assert_eq!(receipt["type"], "read_receipt");
    assert_eq!(receipt["username"], "read-bob");
    assert_eq!(receipt["message_id"], second_id);
    assert_eq!(
        next_json_type(&mut bob, "read_receipt").await["message_id"],
        second_id
    );

    bob.send(Message::Text(
        serde_json::json!({
            "type": "read",
            "message_id": first["message_id"],
        })
        .to_string(),
    ))
    .await
    .unwrap();

    let stored: uuid::Uuid = sqlx::query_scalar("SELECT message_id FROM room_reads")
        .fetch_one(server.state.pool())
        .await
        .unwrap();
    assert_eq!(stored.to_string(), second_id);

    let (_alice_second, auth) = open_room(&server.base, &room_id, &alice_token).await;
    assert_eq!(auth["participants"].as_array().unwrap().len(), 2);
    let bob_receipt = auth["read_receipts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["username"] == "read-bob")
        .unwrap();
    assert_eq!(bob_receipt["message_id"], second_id);
}

#[tokio::test]
async fn editing_is_sender_only_and_typing_carries_the_live_draft() {
    let server = start_server().await;
    let room_id = create_room(&server.base, "editing", "editing-alice").await;
    let alice_token = session_token(&server.base, "editing-alice").await;
    let bob_token = session_token(&server.base, "editing-bob").await;
    let (mut alice, _) = open_room(&server.base, &room_id, &alice_token).await;
    assert_eq!(next_json(&mut alice).await["type"], "presence");
    let (mut bob, _) = open_room(&server.base, &room_id, &bob_token).await;
    assert_eq!(next_json(&mut bob).await["type"], "system");
    assert_eq!(next_json(&mut alice).await["type"], "system");

    send_message(&mut alice, "before edit", None).await;
    let original = next_json_type(&mut alice, "broadcast").await;
    let message_id = original["message_id"].as_str().unwrap();
    let _ = next_json_type(&mut bob, "broadcast").await;

    bob.send(Message::Text(
        serde_json::json!({
            "type": "edit",
            "message_id": message_id,
            "content": "unauthorized"
        })
        .to_string(),
    ))
    .await
    .unwrap();
    tokio::time::sleep(Duration::from_millis(80)).await;
    let stored: String = sqlx::query_scalar("SELECT content FROM messages WHERE id = ?")
        .bind(uuid::Uuid::parse_str(message_id).unwrap())
        .fetch_one(server.state.pool())
        .await
        .unwrap();
    assert_eq!(stored, "before edit");

    alice
        .send(Message::Text(
            serde_json::json!({
                "type": "edit",
                "message_id": message_id,
                "content": "after edit"
            })
            .to_string(),
        ))
        .await
        .unwrap();
    let edited = next_json_type(&mut alice, "message_edited").await;
    assert_eq!(edited["content"], "after edit");
    assert_eq!(
        next_json_type(&mut bob, "message_edited").await["message_id"],
        message_id
    );

    bob.send(Message::Text(
        serde_json::json!({ "type": "typing", "content": "正在写的实时内容" }).to_string(),
    ))
    .await
    .unwrap();
    let typing = next_json_type(&mut alice, "typing").await;
    assert_eq!(typing["username"], "editing-bob");
    assert_eq!(typing["content"], "正在写的实时内容");

    let history: Vec<serde_json::Value> = reqwest::Client::new()
        .get(format!("{}/api/rooms/{room_id}/messages", server.base))
        .bearer_auth(alice_token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(history[0]["content"], "after edit");
    assert!(history[0]["edited_at"].is_string());
}
