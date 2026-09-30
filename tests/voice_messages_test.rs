//! TG-401: sending voice messages — the stored projection, validation, and the TG-505
//! `voice_messages` privacy rule — on SQLite and on PostgreSQL.

mod privacy_support;
mod voice_support;

use chat_room::{config::AppConfig, state::AppState};
use privacy_support::{
    migration_support::{create_postgres_scratch, drop_postgres_scratch, postgres_admin_pool},
    start_server, start_server_on, TestServer,
};
use reqwest::StatusCode;
use voice_support::{
    direct_chat, find, history, next_of, send_voice, waveform, waveform_field, VoiceForm,
};

async fn a_voice_message_carries_its_waveform_and_container_duration(server: &TestServer) {
    let owner = server.account("vm-owner").await;
    let member = server.account("vm-member").await;
    let group = server.create_group(&owner, "vm-group").await;
    server.join(group, &member).await;
    let (mut member_socket, _) = server.open_chat(group, &member).await;

    let form = VoiceForm::webm(3_480, 1);
    let bytes = form.bytes.clone();
    let (status, body) = send_voice(server, group, &owner, form).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(body["media_kind"], "voice");
    assert_eq!(
        body["voice"]["duration_ms"], 3_480,
        "container duration wins"
    );
    assert_eq!(body["voice"]["waveform"], serde_json::json!(waveform()));
    assert_eq!(body["voice"]["listened"], false);
    assert_eq!(body["attachment"]["mime_type"], "audio/webm");
    assert_eq!(body["attachment"]["file_name"], "voice.webm");
    assert_eq!(body["content"], "");

    let frame = next_of(&mut member_socket, "broadcast").await;
    assert_eq!(frame["message_id"], body["id"]);
    assert_eq!(
        frame["voice"], body["voice"],
        "live frame carries the projection"
    );

    let id = body["id"].as_str().unwrap();
    let messages = history(server, group, &member).await;
    let stored = find(&messages, id);
    assert_eq!(stored["voice"]["duration_ms"], 3_480);
    assert_eq!(stored["voice"]["waveform"], serde_json::json!(waveform()));
    assert_eq!(stored["media_kind"], "voice");

    // Forwarding keeps it a voice message (with its waveform), unlistened.
    let target = server.create_group(&member, "vm-target").await;
    let results: Vec<serde_json::Value> = server
        .client
        .post(server.url("/api/messages/forward"))
        .bearer_auth(&member.token)
        .json(&serde_json::json!({ "message_ids": [id], "target_room_ids": [target] }))
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
    assert_eq!(copied["media_kind"], "voice");
    assert_eq!(copied["voice"]["duration_ms"], 3_480);
    assert_eq!(copied["voice"]["waveform"], serde_json::json!(waveform()));
    assert_eq!(copied["voice"]["listened"], false);

    let download = server
        .client
        .get(server.url(body["attachment"]["download_url"].as_str().unwrap()))
        .send()
        .await
        .unwrap();
    assert_eq!(download.status(), StatusCode::OK);
    assert_eq!(download.headers()["content-type"], "audio/webm");
    assert_eq!(download.bytes().await.unwrap().to_vec(), bytes);
}

async fn malformed_voice_messages_are_refused(server: &TestServer) {
    let owner = server.account("vv-owner").await;
    let stranger = server.account("vv-stranger").await;
    let group = server.create_group(&owner, "vv-group").await;

    let short = VoiceForm {
        waveform: waveform_field(&[3; 99]),
        ..VoiceForm::webm(1_000, 2)
    };
    let (status, body) = send_voice(server, group, &owner, short).await;
    assert_eq!(
        (status, &body["error"]),
        (StatusCode::BAD_REQUEST, &"invalid_waveform".into())
    );

    let mut loud = vec![0u8; 100];
    loud[7] = 32;
    let out_of_range = VoiceForm {
        waveform: waveform_field(&loud),
        ..VoiceForm::webm(1_000, 3)
    };
    let (status, _) = send_voice(server, group, &owner, out_of_range).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let html = VoiceForm {
        bytes: b"<html><script>alert(1)</script></html>".to_vec(),
        ..VoiceForm::webm(1_000, 4)
    };
    let (status, body) = send_voice(server, group, &owner, html).await;
    assert_eq!(status, StatusCode::UNSUPPORTED_MEDIA_TYPE, "{body}");

    // An Ogg header with no readable granule: the recorder's duration is the fallback…
    let opaque = || VoiceForm {
        bytes: b"OggS\0\0not-a-real-page".to_vec(),
        mime: "audio/ogg",
        ..VoiceForm::webm(1_000, 5)
    };
    let (status, body) = send_voice(server, group, &owner, opaque()).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(body["voice"]["duration_ms"], 1_000);
    assert_eq!(body["attachment"]["mime_type"], "audio/ogg");
    // …and without one there is no duration at all.
    let (status, body) = send_voice(
        server,
        group,
        &owner,
        VoiceForm {
            duration_ms: None,
            ..opaque()
        },
    )
    .await;
    assert_eq!(
        (status, &body["error"]),
        (StatusCode::BAD_REQUEST, &"missing_duration".into())
    );
    let (status, _) = send_voice(
        server,
        group,
        &owner,
        VoiceForm {
            duration_ms: Some(0),
            ..opaque()
        },
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let (status, _) = send_voice(server, group, &stranger, VoiceForm::webm(1_000, 6)).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "non-members learn nothing");
}

async fn private_chats_honour_the_voice_messages_privacy_rule(server: &TestServer) {
    let alice = server.account("vp-alice").await;
    let bob = server.account("vp-bob").await;
    server.befriend(&alice, &bob).await;
    let direct = direct_chat(server, &alice, &bob).await;
    let group = server.create_group(&alice, "vp-group").await;
    server.join(group, &bob).await;

    let (status, _) = server
        .put_rule(&bob, "voice_messages", "nobody", &[], &[])
        .await;
    assert_eq!(status, StatusCode::OK);
    let (status, body) = send_voice(server, direct, &alice, VoiceForm::webm(900, 10)).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(body["error"], "voice_messages_restricted");
    assert!(
        history(server, direct, &alice)
            .await
            .iter()
            .all(|message| message["voice"].is_null()),
        "a refused voice message leaves nothing behind"
    );

    // The rule is Bob's: Bob may still send to Alice, and groups are not governed by it.
    let (status, _) = send_voice(server, direct, &bob, VoiceForm::webm(900, 11)).await;
    assert_eq!(status, StatusCode::CREATED);
    let (status, _) = send_voice(server, group, &alice, VoiceForm::webm(900, 12)).await;
    assert_eq!(status, StatusCode::CREATED);

    // An allow exception admits Alice even under `nobody`; `contacts` admits a contact.
    let (status, _) = server
        .put_rule(&bob, "voice_messages", "nobody", &[&alice], &[])
        .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = send_voice(server, direct, &alice, VoiceForm::webm(900, 13)).await;
    assert_eq!(status, StatusCode::CREATED);
    let (status, _) = server
        .put_rule(&bob, "voice_messages", "contacts", &[], &[])
        .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = send_voice(server, direct, &alice, VoiceForm::webm(900, 14)).await;
    assert_eq!(status, StatusCode::CREATED);
}

async fn run_all(server: &TestServer) {
    a_voice_message_carries_its_waveform_and_container_duration(server).await;
    malformed_voice_messages_are_refused(server).await;
    private_chats_honour_the_voice_messages_privacy_rule(server).await;
}

#[tokio::test]
async fn sqlite_voice_messages_are_stored_validated_and_privacy_gated() {
    let server = start_server().await;
    run_all(&server).await;
}

#[tokio::test]
async fn postgres_voice_messages_are_stored_validated_and_privacy_gated() {
    let Some((admin_url, admin_pool)) =
        postgres_admin_pool("postgres_voice_messages_are_stored_validated_and_privacy_gated").await
    else {
        return;
    };
    let scratch = create_postgres_scratch(&admin_url, &admin_pool, "voice_messages").await;
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
