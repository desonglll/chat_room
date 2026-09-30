//! Byte-for-byte pins for the frozen WebSocket wire strings.
//!
//! Three frozen clients string-match System frame `content`, disconnect reasons, and
//! `auth_fail` reasons: the Vue web client (`web/src/roomSystemEvents.ts`,
//! `web/src/chatProtocol.ts`), the PySide6 desktop client, and the ratatui client. TG-004's
//! mechanical Room→Chat rename reworded several of these values and silently broke the Vue
//! matchers; the pre-rename spellings are the wire contract (CONTEXT.md, "Room") and were
//! restored. Every assertion below names the frozen client matcher it protects, so the next
//! mechanical rename fails HERE, loudly, instead of in a frozen client at runtime.
//!
//! Two matched strings are not live-triggerable without injecting storage failures and are
//! pinned at their emit sites instead: "message history is temporarily unavailable"
//! (web/src/chatProtocol.ts:47, src/realtime/history_replay.rs) and
//! "message from {username} was not saved or broadcast" (web/src/chatProtocol.ts:48-49,
//! src/realtime/inbound.rs).

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
    let app = build_app(state.clone());
    let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    TestServer {
        base: format!("http://{address}"),
        task,
    }
}

async fn register(base: &str, username: &str) -> (String, String) {
    let response = reqwest::Client::new()
        .post(format!("{base}/api/users/register"))
        .json(&serde_json::json!({ "username": username, "password": "test-password" }))
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();
    (
        response["token"].as_str().unwrap().to_string(),
        response["user"]["id"].as_str().unwrap().to_string(),
    )
}

async fn create_chat(base: &str, name: &str, password: &str, owner_token: &str) -> String {
    reqwest::Client::new()
        .post(format!("{base}/api/chats"))
        .bearer_auth(owner_token)
        .json(&serde_json::json!({ "name": name, "password": password, "join_policy": "open" }))
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

async fn status_of(request: reqwest::RequestBuilder) -> u16 {
    request.send().await.unwrap().status().as_u16()
}

async fn connect_and_send(base: &str, room_id: &str, first_frame: String) -> Socket {
    let url = format!("{}/ws/{room_id}", base.replacen("http://", "ws://", 1));
    let (mut socket, _) = connect_async(url).await.unwrap();
    socket.send(Message::Text(first_frame)).await.unwrap();
    socket
}

/// Joins and waits for `auth_ok`, then for the socket's own join/presence broadcast, which
/// proves the server-side forwarder is live before the test triggers more broadcasts.
async fn open_chat(base: &str, room_id: &str, token: &str, sync_frame: &str) -> Socket {
    let join = serde_json::json!({ "type": "join", "token": token }).to_string();
    let mut socket = connect_and_send(base, room_id, join).await;
    assert_eq!(next_json(&mut socket).await["type"], "auth_ok");
    match sync_frame {
        "presence" => assert_eq!(next_json(&mut socket).await["type"], "presence"),
        expected_system => assert_eq!(next_system_content(&mut socket).await, expected_system),
    }
    socket
}

async fn next_json(socket: &mut Socket) -> serde_json::Value {
    loop {
        let frame = tokio::time::timeout(Duration::from_secs(3), socket.next())
            .await
            .expect("timed out waiting for WebSocket frame")
            .expect("WebSocket ended")
            .expect("WebSocket error");
        let Message::Text(text) = frame else { continue };
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        if value["type"] != "history_complete" && value["type"] != "user_status" {
            return value;
        }
    }
}

async fn next_system_content(socket: &mut Socket) -> String {
    loop {
        let value = next_json(socket).await;
        if value["type"] == "system" {
            return value["content"].as_str().unwrap().to_string();
        }
    }
}

/// Skips unrelated System frames (byte-for-byte comparison; times out loudly if absent).
async fn wait_for_system(socket: &mut Socket, expected: &str) {
    while next_system_content(socket).await != expected {}
}

async fn expect_auth_fail(base: &str, room_id: &str, first_frame: String, expected: &str) {
    let mut socket = connect_and_send(base, room_id, first_frame).await;
    let value = next_json(&mut socket).await;
    assert_eq!(value["type"], "auth_fail");
    assert_eq!(value["reason"], expected);
}

/// `auth_fail` reasons are matched byte-for-byte by the AUTH_ERRORS table in
/// web/src/chatProtocol.ts:22-34 to localise login errors.
#[tokio::test]
async fn auth_fail_reasons_keep_the_frozen_room_spellings() {
    let server = start_server().await;
    let (owner_token, _) = register(&server.base, "pin-auth-owner").await;
    let room_id = create_chat(&server.base, "pin-private", "secret", &owner_token).await;
    let (member_token, _) = register(&server.base, "pin-auth-member").await;
    let join = |token: &str| serde_json::json!({ "type": "join", "token": token }).to_string();

    // web/src/chatProtocol.ts:24 — AUTH_ERRORS['room not found'].
    let missing = uuid::Uuid::new_v4().to_string();
    expect_auth_fail(
        &server.base,
        &missing,
        join(&member_token),
        "room not found",
    )
    .await;

    // Not in AUTH_ERRORS, but a frozen wire value shown verbatim by the clients.
    expect_auth_fail(
        &server.base,
        &room_id,
        join(&member_token),
        "this room requires a password - send auth, not join",
    )
    .await;

    // web/src/chatProtocol.ts:23 — AUTH_ERRORS['wrong password'].
    let bad_auth = serde_json::json!({ "type": "auth", "token": member_token, "password": "no" });
    expect_auth_fail(
        &server.base,
        &room_id,
        bad_auth.to_string(),
        "wrong password",
    )
    .await;

    // web/src/chatProtocol.ts:26 — AUTH_ERRORS['login required'].
    let stale = join(&uuid::Uuid::new_v4().to_string());
    expect_auth_fail(&server.base, &room_id, stale, "login required").await;

    // web/src/chatProtocol.ts:33 — AUTH_ERRORS['invalid json'].
    expect_auth_fail(&server.base, &room_id, "not json".into(), "invalid json").await;

    // web/src/chatProtocol.ts:31-32 and :45-46 — lock reasons reach clients as both
    // auth_fail reasons and System frame content (live path: tests/admin_system_lock_test.rs).
    assert_eq!(
        chat_room::admin_system_lock::SYSTEM_LOCK_REASON,
        "system locked"
    );
    assert_eq!(
        chat_room::admin_system_lock::CHAT_LOCK_REASON,
        "room locked"
    );
}

/// Membership System frames are matched by the regexes in web/src/chatProtocol.ts:37-42
/// and the governance wording is shown verbatim; disconnect reasons close the chat via
/// web/src/roomSystemEvents.ts:34.
#[tokio::test]
async fn membership_system_frames_keep_the_frozen_room_spellings() {
    let server = start_server().await;
    let (owner_token, _) = register(&server.base, "pin-owner").await;
    let room_id = create_chat(&server.base, "pin-members", "", &owner_token).await;
    let mut owner = open_chat(&server.base, &room_id, &owner_token, "presence").await;
    let client = reqwest::Client::new();
    let chat_url = format!("{}/api/chats/{room_id}", server.base);

    // web/src/chatProtocol.ts:37 — /^(.*) joined the room$/.
    let (joiner_token, _) = register(&server.base, "pin-joiner").await;
    let joined = "pin-joiner joined the room";
    let mut joiner = open_chat(&server.base, &room_id, &joiner_token, joined).await;
    assert_eq!(next_system_content(&mut owner).await, joined);

    // web/src/roomSystemEvents.ts:16 prefix 'room renamed to ' and
    // web/src/chatProtocol.ts:41 — /^room renamed to (.*)$/.
    let rename = client
        .patch(&chat_url)
        .bearer_auth(&owner_token)
        .json(&serde_json::json!({ "name": "pin-renamed" }));
    assert_eq!(status_of(rename).await, 200);
    assert_eq!(
        next_system_content(&mut owner).await,
        "room renamed to pin-renamed"
    );

    // web/src/chatProtocol.ts:39 — /^(.*) left the room$/ (broadcast) and
    // web/src/roomSystemEvents.ts:34 — 'membership left' (disconnect reason to the leaver).
    let leave = client
        .delete(format!("{chat_url}/members/me"))
        .bearer_auth(&joiner_token);
    assert_eq!(status_of(leave).await, 204);
    assert_eq!(
        next_system_content(&mut owner).await,
        "pin-joiner left the room"
    );
    wait_for_system(&mut joiner, "membership left").await;

    // Governance wording is a frozen wire value ("room", not "chat"). The targeted
    // disconnect reason 'membership removed' is matched by web/src/roomSystemEvents.ts:34;
    // 'membership banned' predates TG-004 unchanged and no frozen client matches it.
    let (banned_token, banned_id) = register(&server.base, "pin-banned").await;
    let joined = "pin-banned joined the room";
    let mut banned = open_chat(&server.base, &room_id, &banned_token, joined).await;
    assert_eq!(next_system_content(&mut owner).await, joined);
    let ban = client
        .patch(format!("{chat_url}/members/{banned_id}"))
        .bearer_auth(&owner_token)
        .json(&serde_json::json!({ "action": "ban" }));
    assert_eq!(status_of(ban).await, 200);
    assert_eq!(
        next_system_content(&mut owner).await,
        "pin-banned was banned from the room"
    );
    wait_for_system(&mut banned, "membership banned").await;

    let (removed_token, removed_id) = register(&server.base, "pin-removed").await;
    let joined = "pin-removed joined the room";
    let mut removed = open_chat(&server.base, &room_id, &removed_token, joined).await;
    assert_eq!(next_system_content(&mut owner).await, joined);
    let remove = client
        .patch(format!("{chat_url}/members/{removed_id}"))
        .bearer_auth(&owner_token)
        .json(&serde_json::json!({ "action": "remove" }));
    assert_eq!(status_of(remove).await, 200);
    assert_eq!(
        next_system_content(&mut owner).await,
        "pin-removed was removed from the room"
    );
    wait_for_system(&mut removed, "membership removed").await;
}

/// Lifecycle disconnect reasons drive stored-password cleanup, selection cleanup, and the
/// deletion toast in web/src/roomSystemEvents.ts:20 and :25 (localised again by
/// web/src/chatProtocol.ts:43-44).
#[tokio::test]
async fn lifecycle_disconnect_reasons_keep_the_frozen_room_spellings() {
    let server = start_server().await;
    let client = reqwest::Client::new();
    let owner_token = session_token(&server.base, "pin-lifecycle-owner").await;

    // web/src/roomSystemEvents.ts:20 / web/src/chatProtocol.ts:44 — 'room password changed'.
    let room_id = create_chat(&server.base, "pin-password", "old-secret", &owner_token).await;
    let mut socket = open_chat(&server.base, &room_id, &owner_token, "presence").await;
    let change = client
        .patch(format!("{}/api/chats/{room_id}", server.base))
        .bearer_auth(&owner_token)
        .json(&serde_json::json!({
            "current_password": "old-secret",
            "new_password": "new-secret"
        }));
    assert_eq!(status_of(change).await, 200);
    wait_for_system(&mut socket, "room password changed").await;

    // web/src/roomSystemEvents.ts:25 / web/src/chatProtocol.ts:43 — 'room deleted'.
    let room_id = create_chat(&server.base, "pin-delete", "", &owner_token).await;
    let mut socket = open_chat(&server.base, &room_id, &owner_token, "presence").await;
    let delete = client
        .delete(format!("{}/api/chats/{room_id}", server.base))
        .bearer_auth(&owner_token);
    assert_eq!(status_of(delete).await, 204);
    wait_for_system(&mut socket, "room deleted").await;
}
