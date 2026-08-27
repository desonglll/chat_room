use std::{sync::Arc, time::Duration};

use chat_room::{build_app, state::AppState};
use chrono::Utc;
use futures_util::{SinkExt, StreamExt};
use reqwest::{Client, StatusCode};
use tokio::net::TcpListener;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use uuid::Uuid;

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

struct Account {
    id: Uuid,
    token: String,
}

async fn start_server() -> TestServer {
    let state = Arc::new(AppState::new().await.unwrap());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let app = build_app(state.clone());
    let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    TestServer { base, state, task }
}

async fn register(client: &Client, base: &str, username: &str) -> Account {
    let value: serde_json::Value = client
        .post(format!("{base}/api/users/register"))
        .json(&serde_json::json!({ "username": username, "password": "test-password" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    Account {
        id: value["user"]["id"].as_str().unwrap().parse().unwrap(),
        token: value["token"].as_str().unwrap().into(),
    }
}

async fn account_socket(base: &str, token: &str) -> Socket {
    let url = format!("{}/ws/account", base.replacen("http://", "ws://", 1));
    let (mut socket, _) = connect_async(url).await.unwrap();
    socket
        .send(Message::Text(
            serde_json::json!({ "token": token }).to_string(),
        ))
        .await
        .unwrap();
    socket
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
    .unwrap_or_else(|_| panic!("timed out waiting for account event {expected}"))
}

async fn notifications(
    client: &Client,
    base: &str,
    token: &str,
    kind: Option<&str>,
) -> serde_json::Value {
    let mut request = client
        .get(format!("{base}/api/notifications"))
        .bearer_auth(token);
    if let Some(kind) = kind {
        request = request.query(&[("kind", kind)]);
    }
    request.send().await.unwrap().json().await.unwrap()
}

#[tokio::test]
async fn friend_notifications_are_deduplicated_scoped_and_reflected_on_the_account_socket() {
    let server = start_server().await;
    let client = Client::new();
    let alice = register(&client, &server.base, "notify-friend-alice").await;
    let bob = register(&client, &server.base, "notify-friend-bob").await;
    let mut bob_socket = account_socket(&server.base, &bob.token).await;
    let initial = next_type(&mut bob_socket, "notifications_changed").await;
    assert_eq!(initial["unread_count"], 0);

    let endpoint = format!("{}/api/friend-requests", server.base);
    assert_eq!(
        client
            .post(&endpoint)
            .bearer_auth(&alice.token)
            .json(&serde_json::json!({ "user_id": bob.id }))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::CREATED
    );
    client
        .post(&endpoint)
        .bearer_auth(&alice.token)
        .json(&serde_json::json!({ "user_id": bob.id }))
        .send()
        .await
        .unwrap();

    let changed = next_type(&mut bob_socket, "notifications_changed").await;
    assert_eq!(changed["unread_count"], 1);
    assert!(changed["latest_notification_id"].is_string());
    let page = notifications(&client, &server.base, &bob.token, Some("friend_request")).await;
    assert_eq!(page["items"].as_array().unwrap().len(), 1);
    let item = &page["items"][0];
    assert_eq!(item["kind"], "friend_request");
    assert_eq!(item["actor"]["id"], alice.id.to_string());
    assert_eq!(item["source_available"], true);
    let id = item["id"].as_str().unwrap();

    assert_eq!(
        client
            .post(format!("{}/api/notifications/{id}/read", server.base))
            .bearer_auth(&alice.token)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        client
            .post(format!("{}/api/notifications/{id}/read", server.base))
            .bearer_auth(&bob.token)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::NO_CONTENT
    );
    let unread: serde_json::Value = client
        .get(format!("{}/api/notifications/unread-count", server.base))
        .bearer_auth(&bob.token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(unread["unread_count"], 0);
}

#[tokio::test]
async fn message_notifications_reauthorize_and_redact_recalled_sources() {
    let server = start_server().await;
    let client = Client::new();
    let alice = register(&client, &server.base, "notify-message-alice").await;
    let bob = register(&client, &server.base, "notify-message-bob").await;
    let room: serde_json::Value = client
        .post(format!("{}/api/rooms", server.base))
        .bearer_auth(&alice.token)
        .json(&serde_json::json!({ "name": "notification-room", "join_policy": "approval" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let room_id: Uuid = room["id"].as_str().unwrap().parse().unwrap();
    assert_eq!(
        client
            .post(format!("{}/api/rooms/{room_id}/join-requests", server.base))
            .bearer_auth(&bob.token)
            .json(&serde_json::json!({}))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::ACCEPTED
    );
    let join = notifications(
        &client,
        &server.base,
        &alice.token,
        Some("room_join_request"),
    )
    .await;
    assert_eq!(join["items"][0]["actor"]["id"], bob.id.to_string());
    assert_eq!(join["items"][0]["room_id"], room_id.to_string());
    assert_eq!(
        client
            .patch(format!(
                "{}/api/rooms/{room_id}/members/{}",
                server.base, bob.id
            ))
            .bearer_auth(&alice.token)
            .json(&serde_json::json!({ "action": "approve" }))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );

    let original_id = Uuid::new_v4();
    let reply_id = Uuid::new_v4();
    let now = Utc::now();
    sqlx::query(
        "INSERT INTO messages (id, room_id, sender_id, sender, content, created_at) \
         VALUES ($1, $2, $3, 'notify-message-alice', 'confidential mention', $4)",
    )
    .bind(original_id)
    .bind(room_id)
    .bind(alice.id)
    .bind(now)
    .execute(server.state.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO message_mentions (message_id, mentioned_user_id, created_at) \
         VALUES ($1, $2, $3)",
    )
    .bind(original_id)
    .bind(bob.id)
    .bind(now)
    .execute(server.state.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO messages \
         (id, room_id, sender_id, sender, content, reply_to_id, created_at) \
         VALUES ($1, $2, $3, 'notify-message-bob', 'confidential reply', $4, $5)",
    )
    .bind(reply_id)
    .bind(room_id)
    .bind(bob.id)
    .bind(original_id)
    .bind(now + chrono::Duration::milliseconds(1))
    .execute(server.state.pool())
    .await
    .unwrap();

    let mention = notifications(&client, &server.base, &bob.token, Some("mention")).await;
    assert_eq!(mention["items"][0]["summary"], "confidential mention");
    assert_eq!(mention["items"][0]["source_available"], true);
    let reply = notifications(&client, &server.base, &alice.token, Some("reply")).await;
    assert_eq!(reply["items"][0]["summary"], "confidential reply");
    assert_eq!(reply["items"][0]["source_available"], true);

    sqlx::query("UPDATE messages SET recalled_at = $1 WHERE id IN ($2, $3)")
        .bind(Utc::now())
        .bind(original_id)
        .bind(reply_id)
        .execute(server.state.pool())
        .await
        .unwrap();
    let mention = notifications(&client, &server.base, &bob.token, Some("mention")).await;
    assert_eq!(mention["items"][0]["source_available"], false);
    assert_eq!(
        mention["items"][0]["summary"],
        "Source is no longer available"
    );
    assert!(mention["items"][0]["message_id"].is_null());
    let reply = notifications(&client, &server.base, &alice.token, Some("reply")).await;
    assert_eq!(reply["items"][0]["source_available"], false);
    assert!(reply["items"][0]["actor"].is_null());

    let thread_id = Uuid::new_v4();
    let user_message_id = Uuid::new_v4();
    let assistant_message_id = Uuid::new_v4();
    let run_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO ai_threads (id, user_id, title, room_id, created_at, updated_at) \
         VALUES ($1, $2, 'notification run', $3, $4, $4)",
    )
    .bind(thread_id)
    .bind(alice.id)
    .bind(room_id)
    .bind(Utc::now())
    .execute(server.state.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO ai_thread_messages (id, thread_id, role, content, room_id, created_at) \
         VALUES ($1, $2, 'user', 'question', $3, $4), \
                ($5, $2, 'assistant', 'answer', $3, $4)",
    )
    .bind(user_message_id)
    .bind(thread_id)
    .bind(room_id)
    .bind(Utc::now())
    .bind(assistant_message_id)
    .execute(server.state.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO ai_runs \
         (id, thread_id, user_id, user_message_id, assistant_message_id, client_request_id, \
          room_id, status, created_at, updated_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, 'running', $8, $8)",
    )
    .bind(run_id)
    .bind(thread_id)
    .bind(alice.id)
    .bind(user_message_id)
    .bind(assistant_message_id)
    .bind(Uuid::new_v4())
    .bind(room_id)
    .bind(Utc::now())
    .execute(server.state.pool())
    .await
    .unwrap();
    sqlx::query("UPDATE ai_runs SET status = 'completed', updated_at = $1 WHERE id = $2")
        .bind(Utc::now())
        .bind(run_id)
        .execute(server.state.pool())
        .await
        .unwrap();
    let run = notifications(
        &client,
        &server.base,
        &alice.token,
        Some("ai_run_completed"),
    )
    .await;
    assert_eq!(run["items"][0]["run_id"], run_id.to_string());
    assert_eq!(run["items"][0]["summary"], "AI run completed");
}
