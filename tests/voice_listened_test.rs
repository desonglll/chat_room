//! TG-401: the listened ("played") state, in both directions, on SQLite and PostgreSQL.
//!
//! - listener -> sender: the sender's unlistened dot clears (history `listened` + a live
//!   `voice_listened` frame on the sender's connections);
//! - listener -> listener's other devices: their own dot clears the same way;
//! - nobody else learns who listened (the frame is not delivered to a bystander).

mod privacy_support;
mod voice_support;

use chat_room::{config::AppConfig, state::AppState};
use futures_util::SinkExt;
use privacy_support::{
    migration_support::{create_postgres_scratch, drop_postgres_scratch, postgres_admin_pool},
    start_server, start_server_on, TestServer,
};
use reqwest::StatusCode;
use serde_json::{json, Value};
use tokio_tungstenite::tungstenite::Message;
use voice_support::{
    find, frames_before_marker, history, mark_listened, next_of, send_voice, VoiceForm,
};

fn listened_frames(frames: &[Value]) -> usize {
    frames
        .iter()
        .filter(|frame| frame["type"] == "voice_listened")
        .count()
}

async fn listened_of(
    server: &TestServer,
    room: uuid::Uuid,
    who: &privacy_support::Account,
    id: &str,
) -> Value {
    find(&history(server, room, who).await, id)["voice"]["listened"].clone()
}

async fn listened_state_syncs_both_ways(server: &TestServer) {
    let sender = server.account("vl-sender").await;
    let listener = server.account("vl-listener").await;
    let bystander = server.account("vl-bystander").await;
    let outsider = server.account("vl-outsider").await;
    let group = server.create_group(&sender, "vl-group").await;
    server.join(group, &listener).await;
    server.join(group, &bystander).await;

    let (mut sender_socket, _) = server.open_chat(group, &sender).await;
    let (mut listener_phone, _) = server.open_chat(group, &listener).await;
    let (mut listener_desktop, _) = server.open_chat(group, &listener).await;
    let (mut bystander_socket, _) = server.open_chat(group, &bystander).await;

    let (status, body) = send_voice(server, group, &sender, VoiceForm::webm(2_000, 20)).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let id = body["id"].as_str().unwrap().to_string();
    for socket in [
        &mut sender_socket,
        &mut listener_phone,
        &mut listener_desktop,
        &mut bystander_socket,
    ] {
        let frame = next_of(socket, "broadcast").await;
        assert_eq!(frame["voice"]["listened"], false);
    }
    for who in [&sender, &listener, &bystander] {
        assert_eq!(listened_of(server, group, who, &id).await, false);
    }

    // The sender playing their own message changes nothing and tells no one.
    assert_eq!(
        mark_listened(server, &id, &sender).await,
        StatusCode::NO_CONTENT
    );
    let before = frames_before_marker(server, group, &mut sender_socket, "vl-own").await;
    assert_eq!(listened_frames(&before), 0);
    assert_eq!(listened_of(server, group, &sender, &id).await, false);

    // The listener plays it: the sender and the listener's other device hear about it.
    assert_eq!(
        mark_listened(server, &id, &listener).await,
        StatusCode::NO_CONTENT
    );
    for socket in [
        &mut sender_socket,
        &mut listener_phone,
        &mut listener_desktop,
    ] {
        let frame = next_of(socket, "voice_listened").await;
        assert_eq!(frame["message_id"], id.as_str());
        assert_eq!(frame["user_id"], listener.id.to_string());
        assert_eq!(frame["sender_id"], sender.id.to_string());
    }
    let seen = frames_before_marker(server, group, &mut bystander_socket, "vl-by").await;
    assert_eq!(
        listened_frames(&seen),
        0,
        "a bystander never learns who listened: {seen:?}"
    );

    assert_eq!(
        listened_of(server, group, &sender, &id).await,
        true,
        "sender's dot clears"
    );
    assert_eq!(
        listened_of(server, group, &listener, &id).await,
        true,
        "listener's dot clears"
    );
    assert_eq!(
        listened_of(server, group, &bystander, &id).await,
        false,
        "per viewer"
    );

    // Idempotent: a replay neither fails nor re-broadcasts.
    assert_eq!(
        mark_listened(server, &id, &listener).await,
        StatusCode::NO_CONTENT
    );
    let replay = frames_before_marker(server, group, &mut sender_socket, "vl-replay").await;
    assert_eq!(listened_frames(&replay), 0);

    // The existence of a voice message never leaks across the chat boundary.
    assert_eq!(
        mark_listened(server, &id, &outsider).await,
        StatusCode::NOT_FOUND
    );
    let unknown = uuid::Uuid::new_v4().to_string();
    assert_eq!(
        mark_listened(server, &unknown, &listener).await,
        StatusCode::NOT_FOUND
    );
    sender_socket
        .send(Message::Text(
            json!({ "type": "message", "content": "vl-text" }).to_string(),
        ))
        .await
        .unwrap();
    let text = next_of(&mut sender_socket, "broadcast").await["message_id"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(
        mark_listened(server, &text, &listener).await,
        StatusCode::NOT_FOUND
    );
    let anonymous = server
        .client
        .post(server.url(&format!("/api/messages/{id}/voice/listened")))
        .send()
        .await
        .unwrap();
    assert_eq!(anonymous.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn sqlite_listened_state_syncs_both_ways() {
    let server = start_server().await;
    listened_state_syncs_both_ways(&server).await;
}

#[tokio::test]
async fn postgres_listened_state_syncs_both_ways() {
    let Some((admin_url, admin_pool)) =
        postgres_admin_pool("postgres_listened_state_syncs_both_ways").await
    else {
        return;
    };
    let scratch = create_postgres_scratch(&admin_url, &admin_pool, "voice_listened").await;
    let state = AppState::open_postgres(&scratch.url, &AppConfig::default())
        .await
        .unwrap();
    {
        let server = start_server_on(state).await;
        listened_state_syncs_both_ways(&server).await;
        server.state.postgres_pool().unwrap().close().await;
    }
    drop_postgres_scratch(&admin_pool, &scratch).await;
}
