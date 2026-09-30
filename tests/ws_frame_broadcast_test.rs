//! Live behaviour of the TG-007 frame extensions: typing actions travel through the server,
//! `user_status` follows connections, and `auth_ok.statuses` snapshots per-user presence.

use std::time::Duration;

use futures_util::SinkExt;
use tokio_tungstenite::tungstenite::Message;

mod ws_frame_support;
use ws_frame_support::{collect_until, create_chat, open_chat, start_server, user_id};

#[tokio::test]
async fn typing_actions_are_rebroadcast_and_default_for_legacy_frames() {
    let server = start_server().await;
    let room_id = create_chat(&server.base, "typing-actions", "ta-alice").await;
    let alice_token = ws_frame_support::session_token(&server.base, "ta-alice").await;
    let bob_token = ws_frame_support::session_token(&server.base, "ta-bob").await;
    let (mut alice, _) = open_chat(&server.base, room_id, &alice_token).await;
    let (mut bob, _) = open_chat(&server.base, room_id, &bob_token).await;

    bob.send(Message::Text(
        serde_json::json!({ "type": "typing", "content": "dra", "action": "recording_voice" })
            .to_string(),
    ))
    .await
    .unwrap();
    let (typing, _) = collect_until(&mut alice, "typing").await;
    assert_eq!(typing["action"], "recording_voice");
    assert_eq!(typing["content"], "dra");
    assert_eq!(typing["username"], "ta-bob");

    // A pre-TG-007 frame without `action` means plain typing.
    bob.send(Message::Text(
        serde_json::json!({ "type": "typing", "content": "dra" }).to_string(),
    ))
    .await
    .unwrap();
    let (typing, _) = collect_until(&mut alice, "typing").await;
    assert_eq!(typing["action"], "typing");

    // Sending the message clears the draft with an explicit cancel.
    bob.send(Message::Text(
        serde_json::json!({ "type": "message", "content": "done" }).to_string(),
    ))
    .await
    .unwrap();
    let (cleared, _) = collect_until(&mut alice, "typing").await;
    assert_eq!(cleared["action"], "cancel");
    assert_eq!(cleared["content"], "");
    let (broadcast, _) = collect_until(&mut alice, "broadcast").await;
    assert_eq!(broadcast["content"], "done");
}

#[tokio::test]
async fn user_status_frames_follow_connections() {
    let server = start_server().await;
    let room_id = create_chat(&server.base, "user-status", "us-alice").await;
    let alice_token = ws_frame_support::session_token(&server.base, "us-alice").await;
    let bob_token = ws_frame_support::session_token(&server.base, "us-bob").await;
    let alice_id = user_id(&server.state, &alice_token).await;
    let bob_id = user_id(&server.state, &bob_token).await;

    let (mut alice, _) = open_chat(&server.base, room_id, &alice_token).await;
    let (own_status, _) = collect_until(&mut alice, "user_status").await;
    assert_eq!(own_status["user_id"], alice_id.to_string());
    assert_eq!(own_status["status"]["kind"], "online");

    let (bob_socket, _) = open_chat(&server.base, room_id, &bob_token).await;
    let (bob_online, _) = collect_until(&mut alice, "user_status").await;
    assert_eq!(bob_online["user_id"], bob_id.to_string());
    assert_eq!(bob_online["status"]["kind"], "online");

    drop(bob_socket);
    let (bob_offline, _) = collect_until(&mut alice, "user_status").await;
    assert_eq!(bob_offline["user_id"], bob_id.to_string());
    assert_eq!(bob_offline["status"]["kind"], "offline");
    let last_seen = bob_offline["status"]["last_seen"].as_str().unwrap();
    let last_seen = chrono::DateTime::parse_from_rfc3339(last_seen).unwrap();
    assert!((chrono::Utc::now() - last_seen.with_timezone(&chrono::Utc)).num_seconds() < 10);
}

#[tokio::test]
/// TG-505 replaced TG-007's `empty` placeholder: a participant who has connected before now
/// reports the persisted last-seen (the default `everybody` rule admits every viewer).
async fn auth_ok_reports_online_and_persisted_last_seen_per_participant() {
    let server = start_server().await;
    let room_id = create_chat(&server.base, "statuses", "st-alice").await;
    let alice_token = ws_frame_support::session_token(&server.base, "st-alice").await;
    let bob_token = ws_frame_support::session_token(&server.base, "st-bob").await;
    let alice_id = user_id(&server.state, &alice_token).await;
    let bob_id = user_id(&server.state, &bob_token).await;

    // Bob joins once and disconnects: an active participant with no live connection.
    let (bob_socket, _) = open_chat(&server.base, room_id, &bob_token).await;
    drop(bob_socket);
    tokio::time::sleep(Duration::from_millis(80)).await;

    let (_alice, auth) = open_chat(&server.base, room_id, &alice_token).await;
    let statuses = auth["statuses"].as_array().unwrap();
    assert_eq!(
        statuses.len(),
        2,
        "one entry per active participant: {statuses:?}"
    );
    let of = |id: uuid::Uuid| {
        statuses
            .iter()
            .find(|entry| entry["user_id"] == id.to_string())
            .unwrap_or_else(|| panic!("no status entry for {id}"))["status"]
            .clone()
    };
    assert_eq!(of(alice_id)["kind"], "online");
    assert_eq!(of(bob_id)["kind"], "offline");
    assert!(of(bob_id)["last_seen"].is_string());
}
