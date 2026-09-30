//! TG-402: sending round video messages — the stored projection, validation, forwarding and
//! TG-505's "voice messages" privacy rule (which Telegram applies to video messages too) — on
//! SQLite and on PostgreSQL.

mod privacy_support;
mod video_note_support;
mod voice_support;

use base64::Engine;
use chat_room::{config::AppConfig, state::AppState};
use privacy_support::{
    migration_support::{create_postgres_scratch, drop_postgres_scratch, postgres_admin_pool},
    start_server, start_server_on, TestServer,
};
use reqwest::StatusCode;
use serde_json::json;
use video_note_support::{
    send_video_note, VideoNoteForm, JPEG, REAL_WEBM, REAL_WEBM_MS, VOICE_WEBM,
};
use voice_support::{direct_chat, find, history, next_of};

async fn a_video_note_carries_its_duration_and_thumbnail(server: &TestServer) {
    let owner = server.account("vn-owner").await;
    let member = server.account("vn-member").await;
    let group = server.create_group(&owner, "vn-group").await;
    server.join(group, &member).await;
    let (mut member_socket, _) = server.open_chat(group, &member).await;
    let earlier = server.send_message(group, &owner, "before").await;

    let form = VideoNoteForm {
        reply_to: Some(earlier.to_string()),
        ..VideoNoteForm::real()
    };
    let (status, body) = send_video_note(server, group, &owner, form).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(body["media_kind"], "video_note");
    assert_eq!(
        body["video_note"]["duration_ms"], REAL_WEBM_MS,
        "container duration wins over the recorder's 2500"
    );
    let thumbnail = base64::engine::general_purpose::STANDARD.encode(JPEG);
    assert_eq!(body["video_note"]["thumbnail"], thumbnail.as_str());
    assert_eq!(body["video_note"]["listened"], false);
    assert_eq!(body["attachment"]["mime_type"], "video/webm");
    assert_eq!(body["attachment"]["file_name"], "video_note.webm");
    assert_eq!(body["reply_to"]["message_id"], earlier.to_string());
    assert!(body.get("voice").is_none());

    let frame = next_of(&mut member_socket, "broadcast").await;
    assert_eq!(frame["message_id"], body["id"]);
    assert_eq!(frame["video_note"], body["video_note"]);
    assert_eq!(frame["media_kind"], "video_note");

    let id = body["id"].as_str().unwrap();
    let messages = history(server, group, &member).await;
    let stored = find(&messages, id);
    assert_eq!(stored["video_note"]["duration_ms"], REAL_WEBM_MS);
    assert_eq!(stored["video_note"]["thumbnail"], thumbnail.as_str());
    // A second read comes from the history cache (when one is configured): same projection.
    let again = history(server, group, &member).await;
    assert_eq!(find(&again, id)["video_note"], stored["video_note"]);

    // Forwarding keeps it a video note (with its thumbnail), unwatched.
    let target = server.create_group(&member, "vn-target").await;
    let results: Vec<serde_json::Value> = server
        .client
        .post(server.url("/api/messages/forward"))
        .bearer_auth(&member.token)
        .json(&json!({ "message_ids": [id], "target_room_ids": [target] }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let copy = results[0]["forwarded_message_id"]
        .as_str()
        .expect("forwarded");
    let copied = history(server, target, &member).await;
    let copied = find(&copied, copy);
    assert_eq!(copied["media_kind"], "video_note");
    assert_eq!(copied["video_note"]["duration_ms"], REAL_WEBM_MS);
    assert_eq!(copied["video_note"]["thumbnail"], thumbnail.as_str());
    assert_eq!(copied["video_note"]["listened"], false);

    let download = server
        .client
        .get(server.url(body["attachment"]["download_url"].as_str().unwrap()))
        .send()
        .await
        .unwrap();
    assert_eq!(download.status(), StatusCode::OK);
    assert_eq!(download.headers()["content-type"], "video/webm");
    assert_eq!(download.bytes().await.unwrap().to_vec(), REAL_WEBM);

    // Without a thumbnail the field is null, not missing.
    let (status, body) =
        send_video_note(server, group, &owner, VideoNoteForm::synthetic(4_000, 1)).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(body["video_note"]["thumbnail"], serde_json::Value::Null);
    assert_eq!(body["video_note"]["duration_ms"], 4_000);
}

async fn malformed_video_notes_are_refused(server: &TestServer) {
    let owner = server.account("vx-owner").await;
    let stranger = server.account("vx-stranger").await;
    let group = server.create_group(&owner, "vx-group").await;

    // An audio-only recording is a voice message, not a video note.
    let (status, body) = send_video_note(
        server,
        group,
        &owner,
        VideoNoteForm {
            bytes: VOICE_WEBM.to_vec(),
            ..VideoNoteForm::real()
        },
    )
    .await;
    assert_eq!(status, StatusCode::UNSUPPORTED_MEDIA_TYPE);
    assert_eq!(body["error"], "unsupported_video");

    let html = VideoNoteForm {
        bytes: b"<html><script>alert(1)</script></html>".to_vec(),
        ..VideoNoteForm::real()
    };
    let (status, _) = send_video_note(server, group, &owner, html).await;
    assert_eq!(status, StatusCode::UNSUPPORTED_MEDIA_TYPE);

    // Longer than a minute (+1 s of framing grace).
    let (status, body) =
        send_video_note(server, group, &owner, VideoNoteForm::synthetic(61_001, 2)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"], "invalid_duration");
    let (status, body) =
        send_video_note(server, group, &owner, VideoNoteForm::synthetic(60_900, 3)).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");

    // The thumbnail must be a JPEG: a PNG or markup is refused before anything is stored.
    for bogus in [
        privacy_support::PNG.to_vec(),
        b"<svg onload=alert(1)>".to_vec(),
    ] {
        let form = VideoNoteForm {
            thumbnail: Some(bogus),
            ..VideoNoteForm::synthetic(1_000, 4)
        };
        let (status, body) = send_video_note(server, group, &owner, form).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"], "invalid_thumbnail");
    }
    let oversized = VideoNoteForm {
        thumbnail: Some([JPEG, &vec![0; 16 * 1024]].concat()),
        ..VideoNoteForm::synthetic(1_000, 5)
    };
    let (status, _) = send_video_note(server, group, &owner, oversized).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let (status, _) = send_video_note(server, group, &stranger, VideoNoteForm::real()).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "non-members learn nothing");
    assert!(
        history(server, group, &owner)
            .await
            .iter()
            .filter(|message| !message["video_note"].is_null())
            .count()
            == 1,
        "only the 60.9 s note was stored"
    );
}

async fn private_chats_honour_the_voice_messages_privacy_rule(server: &TestServer) {
    let alice = server.account("vnp-alice").await;
    let bob = server.account("vnp-bob").await;
    server.befriend(&alice, &bob).await;
    let direct = direct_chat(server, &alice, &bob).await;

    let (status, _) = server
        .put_rule(&bob, "voice_messages", "nobody", &[], &[])
        .await;
    assert_eq!(status, StatusCode::OK);
    let (status, body) = send_video_note(server, direct, &alice, VideoNoteForm::real()).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(body["error"], "voice_messages_restricted");
    assert!(history(server, direct, &alice)
        .await
        .iter()
        .all(|message| message["video_note"].is_null()));

    let (status, _) = send_video_note(server, direct, &bob, VideoNoteForm::real()).await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "the rule is Bob's, not Alice's"
    );
    let (status, _) = server
        .put_rule(&bob, "voice_messages", "nobody", &[&alice], &[])
        .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = send_video_note(server, direct, &alice, VideoNoteForm::real()).await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "an allow exception admits Alice"
    );
}

async fn run_all(server: &TestServer) {
    a_video_note_carries_its_duration_and_thumbnail(server).await;
    malformed_video_notes_are_refused(server).await;
    private_chats_honour_the_voice_messages_privacy_rule(server).await;
}

#[tokio::test]
async fn sqlite_video_notes_are_stored_validated_and_privacy_gated() {
    let server = start_server().await;
    run_all(&server).await;
}

#[tokio::test]
async fn postgres_video_notes_are_stored_validated_and_privacy_gated() {
    let Some((admin_url, admin_pool)) =
        postgres_admin_pool("postgres_video_notes_are_stored_validated_and_privacy_gated").await
    else {
        return;
    };
    let scratch = create_postgres_scratch(&admin_url, &admin_pool, "video_notes").await;
    let state = AppState::open_postgres(&scratch.url, &AppConfig::default())
        .await
        .unwrap();
    {
        let server = start_server_on(state).await;
        run_all(&server).await;
        server.state.postgres_pool().unwrap().close().await;
    }
    drop_postgres_scratch(&admin_pool, &scratch).await;
}
