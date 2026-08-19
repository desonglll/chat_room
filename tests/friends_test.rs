use std::sync::Arc;

use chat_room::{build_app, state::AppState};
use futures_util::{SinkExt, StreamExt};
use reqwest::{Client, StatusCode};
use tokio::net::TcpListener;
use tokio_tungstenite::{connect_async, tungstenite::Message};

struct Account {
    id: String,
    token: String,
}

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

async fn register(client: &Client, base: &str, username: &str) -> Account {
    let response = client
        .post(format!("{base}/api/users/register"))
        .json(&serde_json::json!({
            "username": username,
            "password": "test-password"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let session: serde_json::Value = response.json().await.unwrap();
    Account {
        id: session["user"]["id"].as_str().unwrap().to_string(),
        token: session["token"].as_str().unwrap().to_string(),
    }
}

async fn next_type(
    socket: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
    expected: &str,
) -> serde_json::Value {
    loop {
        let frame = socket.next().await.unwrap().unwrap();
        let Message::Text(text) = frame else { continue };
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        if value["type"] == expected {
            return value;
        }
    }
}

async fn open_room(
    base: &str,
    room_id: &str,
    token: &str,
    expected_title: &str,
) -> tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>> {
    let url = format!("{}/ws/{room_id}", base.replacen("http://", "ws://", 1));
    let (mut socket, _) = connect_async(url).await.unwrap();
    socket
        .send(Message::Text(
            serde_json::json!({ "type": "join", "token": token }).to_string(),
        ))
        .await
        .unwrap();
    let auth = next_type(&mut socket, "auth_ok").await;
    assert_eq!(auth["room_name"], expected_title);
    socket
}

#[tokio::test]
async fn friends_can_start_one_shared_direct_conversation() {
    let server = start_server().await;
    let client = Client::new();
    let alice = register(&client, &server.base, "friend-alice").await;
    let bob = register(&client, &server.base, "friend-bob").await;

    let search = client
        .get(format!("{}/api/users/search?q=friend-bob", server.base))
        .bearer_auth(&alice.token)
        .send()
        .await
        .unwrap();
    assert_eq!(search.status(), StatusCode::OK);
    let results: Vec<serde_json::Value> = search.json().await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0]["id"], bob.id);
    assert_eq!(results[0]["relationship"], "none");

    let requested = client
        .post(format!("{}/api/friend-requests", server.base))
        .bearer_auth(&alice.token)
        .json(&serde_json::json!({ "user_id": bob.id }))
        .send()
        .await
        .unwrap();
    assert_eq!(requested.status(), StatusCode::CREATED);

    let incoming = client
        .get(format!(
            "{}/api/friend-requests?direction=incoming",
            server.base
        ))
        .bearer_auth(&bob.token)
        .send()
        .await
        .unwrap();
    assert_eq!(incoming.status(), StatusCode::OK);
    let incoming: Vec<serde_json::Value> = incoming.json().await.unwrap();
    assert_eq!(incoming.len(), 1);
    assert_eq!(incoming[0]["user"]["id"], alice.id);
    assert_eq!(incoming[0]["direction"], "incoming");

    let accepted = client
        .patch(format!("{}/api/friend-requests/{}", server.base, alice.id))
        .bearer_auth(&bob.token)
        .json(&serde_json::json!({ "action": "accept" }))
        .send()
        .await
        .unwrap();
    assert_eq!(accepted.status(), StatusCode::OK);

    let alice_chat = client
        .post(format!("{}/api/direct-chats", server.base))
        .bearer_auth(&alice.token)
        .json(&serde_json::json!({ "user_id": bob.id }))
        .send()
        .await
        .unwrap();
    assert_eq!(alice_chat.status(), StatusCode::OK);
    let alice_chat: serde_json::Value = alice_chat.json().await.unwrap();
    assert_eq!(alice_chat["kind"], "direct");
    assert_eq!(alice_chat["title"], "friend-bob");
    assert_eq!(alice_chat["peer"]["id"], bob.id);

    let bob_chat = client
        .post(format!("{}/api/direct-chats", server.base))
        .bearer_auth(&bob.token)
        .json(&serde_json::json!({ "user_id": alice.id }))
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();
    assert_eq!(bob_chat["room_id"], alice_chat["room_id"]);
    assert_eq!(bob_chat["title"], "friend-alice");

    let alice_conversations = client
        .get(format!("{}/api/conversations", server.base))
        .bearer_auth(&alice.token)
        .send()
        .await
        .unwrap()
        .json::<Vec<serde_json::Value>>()
        .await
        .unwrap();
    let bob_conversations = client
        .get(format!("{}/api/conversations", server.base))
        .bearer_auth(&bob.token)
        .send()
        .await
        .unwrap()
        .json::<Vec<serde_json::Value>>()
        .await
        .unwrap();
    assert_eq!(alice_conversations.len(), 1);
    assert_eq!(bob_conversations.len(), 1);
    assert_eq!(alice_conversations[0]["title"], "friend-bob");
    assert_eq!(bob_conversations[0]["title"], "friend-alice");
}

#[tokio::test]
async fn direct_conversation_is_private_and_reuses_room_messaging() {
    let server = start_server().await;
    let client = Client::new();
    let alice = register(&client, &server.base, "private-alice").await;
    let bob = register(&client, &server.base, "private-bob").await;
    let charlie = register(&client, &server.base, "private-charlie").await;

    assert_eq!(
        client
            .post(format!("{}/api/friend-requests", server.base))
            .bearer_auth(&alice.token)
            .json(&serde_json::json!({ "user_id": bob.id }))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::CREATED
    );
    assert_eq!(
        client
            .patch(format!("{}/api/friend-requests/{}", server.base, alice.id))
            .bearer_auth(&bob.token)
            .json(&serde_json::json!({ "action": "accept" }))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    let conversation = client
        .post(format!("{}/api/direct-chats", server.base))
        .bearer_auth(&alice.token)
        .json(&serde_json::json!({ "user_id": bob.id }))
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();
    let room_id = conversation["room_id"].as_str().unwrap();

    let public_rooms = client
        .get(format!("{}/api/rooms", server.base))
        .send()
        .await
        .unwrap()
        .json::<Vec<serde_json::Value>>()
        .await
        .unwrap();
    assert!(public_rooms.iter().all(|room| room["id"] != room_id));
    assert_eq!(
        client
            .get(format!("{}/api/rooms/{room_id}", server.base))
            .bearer_auth(&charlie.token)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::NOT_FOUND
    );
    let alice_room: serde_json::Value = client
        .get(format!("{}/api/rooms/{room_id}", server.base))
        .bearer_auth(&alice.token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(alice_room["name"], "private-bob");
    assert_eq!(
        client
            .post(format!("{}/api/rooms/{room_id}/join-requests", server.base))
            .bearer_auth(&charlie.token)
            .json(&serde_json::json!({}))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::NOT_FOUND
    );

    let mut socket = open_room(&server.base, room_id, &alice.token, "private-bob").await;
    socket
        .send(Message::Text(
            serde_json::json!({ "type": "message", "content": "direct hello" }).to_string(),
        ))
        .await
        .unwrap();
    assert_eq!(
        next_type(&mut socket, "broadcast").await["content"],
        "direct hello"
    );

    let bob_conversations = client
        .get(format!("{}/api/conversations", server.base))
        .bearer_auth(&bob.token)
        .send()
        .await
        .unwrap()
        .json::<Vec<serde_json::Value>>()
        .await
        .unwrap();
    assert_eq!(
        bob_conversations[0]["last_message"]["content"],
        "direct hello"
    );
    assert_eq!(bob_conversations[0]["unread_count"], 1);
}

#[tokio::test]
async fn removing_and_blocking_friendship_control_direct_access() {
    let server = start_server().await;
    let client = Client::new();
    let alice = register(&client, &server.base, "relation-alice").await;
    let bob = register(&client, &server.base, "relation-bob").await;

    client
        .post(format!("{}/api/friend-requests", server.base))
        .bearer_auth(&alice.token)
        .json(&serde_json::json!({ "user_id": bob.id }))
        .send()
        .await
        .unwrap();
    client
        .patch(format!("{}/api/friend-requests/{}", server.base, alice.id))
        .bearer_auth(&bob.token)
        .json(&serde_json::json!({ "action": "accept" }))
        .send()
        .await
        .unwrap();
    let first_chat = client
        .post(format!("{}/api/direct-chats", server.base))
        .bearer_auth(&alice.token)
        .json(&serde_json::json!({ "user_id": bob.id }))
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();
    let room_id = first_chat["room_id"].as_str().unwrap().to_string();

    let friends = client
        .get(format!("{}/api/friends", server.base))
        .bearer_auth(&alice.token)
        .send()
        .await
        .unwrap();
    assert_eq!(friends.status(), StatusCode::OK);
    let friends: Vec<serde_json::Value> = friends.json().await.unwrap();
    assert_eq!(friends[0]["id"], bob.id);
    assert_eq!(friends[0]["relationship"], "friend");

    assert_eq!(
        client
            .delete(format!("{}/api/friends/{}", server.base, bob.id))
            .bearer_auth(&alice.token)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::NO_CONTENT
    );
    for account in [&alice, &bob] {
        let conversations = client
            .get(format!("{}/api/conversations", server.base))
            .bearer_auth(&account.token)
            .send()
            .await
            .unwrap()
            .json::<Vec<serde_json::Value>>()
            .await
            .unwrap();
        assert!(conversations.is_empty());
    }
    assert_eq!(
        client
            .post(format!("{}/api/direct-chats", server.base))
            .bearer_auth(&alice.token)
            .json(&serde_json::json!({ "user_id": bob.id }))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::CONFLICT
    );

    client
        .post(format!("{}/api/friend-requests", server.base))
        .bearer_auth(&alice.token)
        .json(&serde_json::json!({ "user_id": bob.id }))
        .send()
        .await
        .unwrap();
    client
        .patch(format!("{}/api/friend-requests/{}", server.base, alice.id))
        .bearer_auth(&bob.token)
        .json(&serde_json::json!({ "action": "accept" }))
        .send()
        .await
        .unwrap();
    let reopened = client
        .post(format!("{}/api/direct-chats", server.base))
        .bearer_auth(&bob.token)
        .json(&serde_json::json!({ "user_id": alice.id }))
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();
    assert_eq!(reopened["room_id"], room_id);

    assert_eq!(
        client
            .put(format!("{}/api/blocks/{}", server.base, bob.id))
            .bearer_auth(&alice.token)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::NO_CONTENT
    );
    let blocks = client
        .get(format!("{}/api/blocks", server.base))
        .bearer_auth(&alice.token)
        .send()
        .await
        .unwrap()
        .json::<Vec<serde_json::Value>>()
        .await
        .unwrap();
    assert_eq!(blocks[0]["id"], bob.id);
    assert_eq!(blocks[0]["relationship"], "blocked");

    let hidden = client
        .get(format!("{}/api/users/search?q=relation-alice", server.base))
        .bearer_auth(&bob.token)
        .send()
        .await
        .unwrap()
        .json::<Vec<serde_json::Value>>()
        .await
        .unwrap();
    assert!(hidden.is_empty());
    assert_eq!(
        client
            .post(format!("{}/api/friend-requests", server.base))
            .bearer_auth(&bob.token)
            .json(&serde_json::json!({ "user_id": alice.id }))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        client
            .delete(format!("{}/api/blocks/{}", server.base, bob.id))
            .bearer_auth(&alice.token)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::NO_CONTENT
    );
}
