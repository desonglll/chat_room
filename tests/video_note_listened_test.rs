//! TG-402: the watched state of a round video message, on SQLite and PostgreSQL. It reuses
//! TG-401's listened mechanism: the first view by someone else emits `voice_listened` to the
//! viewer's and the sender's own connections only; history carries `video_note.listened` per
//! viewer; nothing leaks across the chat boundary or between the voice and video endpoints.

mod privacy_support;
mod video_note_support;
mod voice_support;

use chat_room::{config::AppConfig, state::AppState};
use privacy_support::{
    migration_support::{create_postgres_scratch, drop_postgres_scratch, postgres_admin_pool},
    start_server, start_server_on, Account, TestServer,
};
use reqwest::StatusCode;
use serde_json::Value;
use video_note_support::{mark_watched, send_video_note, VideoNoteForm};
use voice_support::{
    find, frames_before_marker, history, mark_listened, next_of, send_voice, VoiceForm,
};

fn listened_frames(frames: &[Value]) -> usize {
    frames
        .iter()
        .filter(|frame| frame["type"] == "voice_listened")
        .count()
}

async fn watched_of(server: &TestServer, room: uuid::Uuid, who: &Account, id: &str) -> Value {
    find(&history(server, room, who).await, id)["video_note"]["listened"].clone()
}

async fn watched_state_syncs_both_ways(server: &TestServer) {
    let sender = server.account("vw-sender").await;
    let viewer = server.account("vw-viewer").await;
    let bystander = server.account("vw-bystander").await;
    let outsider = server.account("vw-outsider").await;
    let group = server.create_group(&sender, "vw-group").await;
    server.join(group, &viewer).await;
    server.join(group, &bystander).await;

    let (mut sender_socket, _) = server.open_chat(group, &sender).await;
    let (mut viewer_socket, _) = server.open_chat(group, &viewer).await;
    let (mut bystander_socket, _) = server.open_chat(group, &bystander).await;

    let (status, body) = send_video_note(server, group, &sender, VideoNoteForm::real()).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let id = body["id"].as_str().unwrap().to_string();
    for socket in [
        &mut sender_socket,
        &mut viewer_socket,
        &mut bystander_socket,
    ] {
        assert_eq!(
            next_of(socket, "broadcast").await["video_note"]["listened"],
            false
        );
    }

    // The sender watching their own note changes nothing and tells no one.
    assert_eq!(
        mark_watched(server, &id, &sender).await,
        StatusCode::NO_CONTENT
    );
    let own = frames_before_marker(server, group, &mut sender_socket, "vw-own").await;
    assert_eq!(listened_frames(&own), 0);
    assert_eq!(watched_of(server, group, &sender, &id).await, false);

    assert_eq!(
        mark_watched(server, &id, &viewer).await,
        StatusCode::NO_CONTENT
    );
    for socket in [&mut sender_socket, &mut viewer_socket] {
        let frame = next_of(socket, "voice_listened").await;
        assert_eq!(frame["message_id"], id.as_str());
        assert_eq!(frame["user_id"], viewer.id.to_string());
        assert_eq!(frame["sender_id"], sender.id.to_string());
    }
    let seen = frames_before_marker(server, group, &mut bystander_socket, "vw-by").await;
    assert_eq!(
        listened_frames(&seen),
        0,
        "a bystander never learns who watched"
    );
    assert_eq!(watched_of(server, group, &sender, &id).await, true);
    assert_eq!(watched_of(server, group, &viewer, &id).await, true);
    assert_eq!(
        watched_of(server, group, &bystander, &id).await,
        false,
        "per viewer"
    );

    // Idempotent.
    assert_eq!(
        mark_watched(server, &id, &viewer).await,
        StatusCode::NO_CONTENT
    );
    let replay = frames_before_marker(server, group, &mut sender_socket, "vw-replay").await;
    assert_eq!(listened_frames(&replay), 0);

    // Chat boundary, unknown ids, and the two media kinds' endpoints stay apart.
    assert_eq!(
        mark_watched(server, &id, &outsider).await,
        StatusCode::NOT_FOUND
    );
    let unknown = uuid::Uuid::new_v4().to_string();
    assert_eq!(
        mark_watched(server, &unknown, &viewer).await,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        mark_listened(server, &id, &viewer).await,
        StatusCode::NOT_FOUND
    );
    let (status, voice) = send_voice(server, group, &sender, VoiceForm::webm(1_500, 40)).await;
    assert_eq!(status, StatusCode::CREATED);
    let voice_id = voice["id"].as_str().unwrap();
    assert_eq!(
        mark_watched(server, voice_id, &viewer).await,
        StatusCode::NOT_FOUND
    );
    let anonymous = server
        .client
        .post(server.url(&format!("/api/messages/{id}/video_note/listened")))
        .send()
        .await
        .unwrap();
    assert_eq!(anonymous.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn sqlite_watched_state_syncs_both_ways() {
    let server = start_server().await;
    watched_state_syncs_both_ways(&server).await;
}

#[tokio::test]
async fn postgres_watched_state_syncs_both_ways() {
    let Some((admin_url, admin_pool)) =
        postgres_admin_pool("postgres_watched_state_syncs_both_ways").await
    else {
        return;
    };
    let scratch = create_postgres_scratch(&admin_url, &admin_pool, "video_note_watched").await;
    let state = AppState::open_postgres(&scratch.url, &AppConfig::default())
        .await
        .unwrap();
    {
        let server = start_server_on(state).await;
        watched_state_syncs_both_ways(&server).await;
        server.state.postgres_pool().unwrap().close().await;
    }
    drop_postgres_scratch(&admin_pool, &scratch).await;
}
