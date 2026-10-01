//! TG-907: the terminal client's contacts / pins / forward bindings against a real in-process
//! server — proves the TUI speaks the same wire format as the Web client.

use std::sync::Arc;

use chat_room::{build_app, config::AppConfig, state::AppState};
use tokio::net::TcpListener;

use crate::client_api::ApiClient;

async fn server() -> (String, tokio::task::JoinHandle<()>) {
    let state = Arc::new(
        AppState::new_with_config(&AppConfig::default())
            .await
            .unwrap(),
    );
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move { axum::serve(listener, build_app(state)).await.unwrap() });
    (base, task)
}

async fn account(base: &str, name: &str) -> ApiClient {
    let session = ApiClient::new(base, None)
        .authenticate(true, name, "correct-horse-7")
        .await
        .unwrap();
    ApiClient::new(base, Some(session.token))
}

#[tokio::test]
async fn contacts_pins_and_forward_round_trip() {
    let (base, task) = server().await;
    let alice = account(&base, "tui_alice").await;
    let bob = account(&base, "tui_bob").await;

    // Add by username → Bob sees the request → accepts → both are friends with a status.
    assert_eq!(
        alice.add_contact("@tui_bob").await.unwrap(),
        "Request sent to @tui_bob"
    );
    let requests = bob.incoming_requests().await.unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].user.username, "tui_alice");
    bob.respond_request(requests[0].user.id, true)
        .await
        .unwrap();
    let friends = alice.friends().await.unwrap();
    assert_eq!(friends.len(), 1);
    assert_eq!(friends[0].name(), "tui_bob");
    let bob_id = friends[0].id;
    let statuses = alice.friend_statuses().await.unwrap();
    assert_eq!(statuses.len(), 1);
    assert!(statuses[0].status["kind"].is_string());
    assert_eq!(
        alice.add_contact("tui_bob").await.unwrap(),
        "@tui_bob is already a contact"
    );
    assert_eq!(
        alice.add_contact("nobody_here").await.unwrap(),
        "No user @nobody_here"
    );

    // Open the private chat, pin a message, read it back, unpin.
    let room = alice.open_direct_chat(bob_id).await.unwrap();
    let alice_id = bob.friends().await.unwrap()[0].id;
    assert_eq!(bob.open_direct_chat(alice_id).await.unwrap(), room);
    let message = post(&base, &alice, room, "pin me").await;
    alice.set_pinned(room, message, true).await.unwrap();
    let pins = bob.pins(room).await.unwrap();
    assert_eq!(pins.len(), 1);
    assert_eq!(pins[0].message.content, "pin me");
    alice.set_pinned(room, message, false).await.unwrap();
    assert!(bob.pins(room).await.unwrap().is_empty());

    // Forward into a group Alice created.
    let group = create_group(&base, &alice, "tui forward target").await;
    let result = alice.forward(message, group).await.unwrap();
    assert!(result.forwarded_message_id.is_some(), "{result:?}");
    task.abort();
}

fn client_token(client: &ApiClient) -> uuid::Uuid {
    client.token().unwrap()
}

/// One text message through the chat WebSocket (the TUI's own send path), waiting for its echo.
async fn post(base: &str, client: &ApiClient, room: uuid::Uuid, content: &str) -> uuid::Uuid {
    use futures_util::{SinkExt, StreamExt};
    use tokio_tungstenite::{connect_async, tungstenite::Message};
    let (mut socket, _) = connect_async(format!("{}/ws/{room}", base.replacen("http", "ws", 1)))
        .await
        .unwrap();
    let join = serde_json::json!({ "type": "join", "token": client_token(client) });
    socket.send(Message::Text(join.to_string())).await.unwrap();
    let message = serde_json::json!({ "type": "message", "content": content });
    socket
        .send(Message::Text(message.to_string()))
        .await
        .unwrap();
    while let Some(Ok(frame)) = socket.next().await {
        if let Message::Text(text) = frame {
            let frame: serde_json::Value = serde_json::from_str(&text).unwrap();
            if frame["type"] == "broadcast" && frame["content"] == content {
                return frame["message_id"].as_str().unwrap().parse().unwrap();
            }
        }
    }
    panic!("no echo for {content}");
}

async fn create_group(base: &str, client: &ApiClient, title: &str) -> uuid::Uuid {
    let chat: serde_json::Value = reqwest::Client::new()
        .post(format!("{base}/api/chats"))
        .bearer_auth(client_token(client))
        .json(&serde_json::json!({ "title": title, "password": "", "join_policy": "open" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    chat["id"].as_str().unwrap().parse().unwrap()
}
