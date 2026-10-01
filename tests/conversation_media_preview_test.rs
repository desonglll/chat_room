//! TG-802: the chat list's last-message preview names its media kind, and forwarding a voice
//! message into a private chat obeys the recipient's `voice_messages` rule — on SQLite and on
//! PostgreSQL.

mod privacy_support;
mod voice_support;

use chat_room::{config::AppConfig, state::AppState};
use privacy_support::{
    migration_support::{create_postgres_scratch, drop_postgres_scratch, postgres_admin_pool},
    start_server, start_server_on, Account, TestServer,
};
use reqwest::StatusCode;
use serde_json::{json, Value};
use uuid::Uuid;
use voice_support::{direct_chat, send_voice, VoiceForm};

async fn last_message(server: &TestServer, account: &Account, room_id: Uuid) -> Value {
    let conversations: Value = server
        .client
        .get(server.url("/api/conversations"))
        .bearer_auth(&account.token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    conversations
        .as_array()
        .unwrap()
        .iter()
        .find(|conversation| conversation["room_id"] == room_id.to_string())
        .unwrap_or_else(|| panic!("{room_id} missing from {conversations}"))["last_message"]
        .clone()
}

async fn forward(server: &TestServer, account: &Account, message_id: &str, target: Uuid) -> Value {
    let response = server
        .client
        .post(server.url("/api/messages/forward"))
        .bearer_auth(&account.token)
        .json(&json!({ "message_ids": [message_id], "target_room_ids": [target] }))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let results: Value = response.json().await.unwrap();
    results[0].clone()
}

async fn previews_name_the_media_kind(server: &TestServer) {
    let alice = server.account("mp-alice").await;
    let group = server.create_group(&alice, "mp-group").await;

    server.send_message(group, &alice, "plain words").await;
    let preview = last_message(server, &alice, group).await;
    assert_eq!(preview["content"], "plain words");
    assert!(
        preview.get("media_kind").is_none(),
        "text has no kind: {preview}"
    );

    let (status, body) = send_voice(server, group, &alice, VoiceForm::webm(1_200, 21)).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let preview = last_message(server, &alice, group).await;
    assert_eq!(preview["media_kind"], "voice", "{preview}");

    let response = server
        .client
        .post(server.url(&format!("/api/chats/{group}/polls")))
        .bearer_auth(&alice.token)
        .json(&json!({ "question": "Lunch?", "options": ["Yes", "No"] }))
        .send()
        .await
        .unwrap();
    assert!(response.status().is_success(), "{}", response.status());
    let preview = last_message(server, &alice, group).await;
    assert_eq!(preview["media_kind"], "poll", "{preview}");
    assert_eq!(preview["content"], "Lunch?");
}

async fn forwarded_voice_obeys_the_recipients_privacy(server: &TestServer) {
    let alice = server.account("fv-alice").await;
    let bob = server.account("fv-bob").await;
    server.befriend(&alice, &bob).await;
    let direct = direct_chat(server, &alice, &bob).await;
    let group = server.create_group(&alice, "fv-group").await;
    let other_group = server.create_group(&alice, "fv-other").await;

    let (status, voice) = send_voice(server, group, &alice, VoiceForm::webm(900, 22)).await;
    assert_eq!(status, StatusCode::CREATED, "{voice}");
    let voice_id = voice["id"].as_str().unwrap().to_string();
    let text_id = server
        .send_message(group, &alice, "words")
        .await
        .to_string();

    let (status, _) = server
        .put_rule(&bob, "voice_messages", "nobody", &[], &[])
        .await;
    assert_eq!(status, StatusCode::OK);

    let refused = forward(server, &alice, &voice_id, direct).await;
    assert_eq!(
        refused["skipped_reason"], "voice_messages_restricted",
        "{refused}"
    );
    assert!(refused["forwarded_message_id"].is_null());
    assert!(
        last_message(server, &bob, direct).await.is_null(),
        "a refused forward leaves nothing in the private chat"
    );

    // Text is not governed by the rule, and groups are not either.
    let text = forward(server, &alice, &text_id, direct).await;
    assert!(text["forwarded_message_id"].is_string(), "{text}");
    let grouped = forward(server, &alice, &voice_id, other_group).await;
    assert!(grouped["forwarded_message_id"].is_string(), "{grouped}");
    assert_eq!(
        last_message(server, &alice, other_group).await["media_kind"],
        "voice"
    );

    // An allow exception admits Alice again.
    let (status, _) = server
        .put_rule(&bob, "voice_messages", "nobody", &[&alice], &[])
        .await;
    assert_eq!(status, StatusCode::OK);
    let admitted = forward(server, &alice, &voice_id, direct).await;
    assert!(admitted["forwarded_message_id"].is_string(), "{admitted}");
    assert_eq!(
        last_message(server, &bob, direct).await["media_kind"],
        "voice"
    );
}

async fn run_all(server: &TestServer) {
    previews_name_the_media_kind(server).await;
    forwarded_voice_obeys_the_recipients_privacy(server).await;
}

#[tokio::test]
async fn sqlite_previews_carry_media_kind_and_forwards_obey_voice_privacy() {
    let server = start_server().await;
    run_all(&server).await;
}

#[tokio::test]
async fn postgres_previews_carry_media_kind_and_forwards_obey_voice_privacy() {
    let Some((admin_url, admin_pool)) =
        postgres_admin_pool("postgres_previews_carry_media_kind_and_forwards_obey_voice_privacy")
            .await
    else {
        return;
    };
    let scratch = create_postgres_scratch(&admin_url, &admin_pool, "media_preview").await;
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
