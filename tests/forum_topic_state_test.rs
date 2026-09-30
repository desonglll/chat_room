//! TG-204: per-topic unread and mute, independent of the chat-level cursor and mute, and
//! topic deletion (the topic's messages go, attachments are orphaned unless still referenced).
//! SQLite always, PostgreSQL when configured.

use std::sync::Arc;

use chat_room::state::AppState;
use reqwest::{Method, StatusCode};
use serde_json::{json, Value};
use uuid::Uuid;

mod chat_admin_support;
mod forum_support;

use chat_admin_support::{next_frame, serve, with_postgres, Account, Server};
use forum_support::*;

async fn unread(server: &Server, chat: &str, account: &Account, title: &str) -> (i64, bool) {
    let topic = topic(server, chat, account, title).await;
    (
        topic["unread_count"].as_i64().unwrap(),
        topic["muted"].as_bool().unwrap(),
    )
}

async fn read(
    server: &Server,
    chat: &str,
    topic: &str,
    account: &Account,
    message: &Value,
) -> Value {
    let (status, result) = server
        .call(
            Method::POST,
            &format!("/api/chats/{chat}/topics/{topic}/read"),
            &account.token,
            Some(json!({ "message_id": message["message_id"] })),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{result}");
    result
}

async fn orphaned(state: &AppState, attachment: &str) -> bool {
    let id = Uuid::parse_str(attachment).unwrap();
    let query = "SELECT orphaned_at IS NOT NULL FROM attachments WHERE id = $1";
    match state.postgres_pool() {
        Some(pool) => sqlx::query_scalar(query).bind(id).fetch_one(pool).await,
        None => {
            sqlx::query_scalar(query)
                .bind(id)
                .fetch_one(state.pool())
                .await
        }
    }
    .unwrap()
}

async fn unread_and_mute_scenario(state: Arc<AppState>) {
    let server = serve(state.clone()).await;
    let owner = server.register("tg204-reader").await;
    let member = server.register("tg204-writer").await;
    let chat = server.create_group(&owner, "tg204-unread").await;
    server.join(&chat, &member).await;
    enable_forum(&server, &chat, &owner).await;
    let alpha = new_topic(&server, &chat, &owner, "Alpha").await;
    let beta = new_topic(&server, &chat, &owner, "Beta").await;
    let mut socket = server.socket(&chat, &member).await;
    let mut sent = Vec::new();
    for (content, topic) in [("a1", &alpha), ("a2", &alpha), ("b1", &beta)] {
        send_in_topic(&mut socket, content, Some(topic)).await;
        sent.push(broadcast_of(&mut socket, content).await);
    }
    assert_eq!(unread(&server, &chat, &owner, "Alpha").await, (2, false));
    assert_eq!(unread(&server, &chat, &owner, "Beta").await, (1, false));
    assert_eq!(unread(&server, &chat, &owner, "General").await, (0, false));
    assert_eq!(
        unread(&server, &chat, &member, "Alpha").await.0,
        0,
        "own messages are read"
    );
    assert_eq!(chat_unread(&server, &chat, &owner.token).await, 3);

    // Reading Alpha clears Alpha only; the chat-level cursor does not move.
    let result = read(&server, &chat, &alpha, &owner, &sent[1]).await;
    assert_eq!(result["unread_count"], 0);
    assert_eq!(result["chat_read_advanced"], false);
    assert_eq!(unread(&server, &chat, &owner, "Beta").await.0, 1);
    assert_eq!(chat_unread(&server, &chat, &owner.token).await, 3);
    // Reading backwards never rewinds; a message of another topic is refused.
    read(&server, &chat, &alpha, &owner, &sent[0]).await;
    assert_eq!(unread(&server, &chat, &owner, "Alpha").await.0, 0);
    let (status, _) = server
        .call(
            Method::POST,
            &format!("/api/chats/{chat}/topics/{beta}/read"),
            &owner.token,
            Some(json!({ "message_id": sent[0]["message_id"] })),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    // The last unread topic read: the chat-level cursor follows.
    let result = read(&server, &chat, &beta, &owner, &sent[2]).await;
    assert_eq!(result["chat_read_advanced"], true);
    assert_eq!(chat_unread(&server, &chat, &owner.token).await, 0);

    // Mute: per topic, independent of the chat's mute in both directions.
    let (status, muted) = server
        .put(
            &format!("/api/chats/{chat}/topics/{alpha}/notifications"),
            &owner.token,
            json!({ "muted": true, "muted_until": null }),
        )
        .await;
    assert_eq!(
        (status, muted["muted"].clone()),
        (StatusCode::OK, json!(true))
    );
    let (_, preferences) = server
        .get(
            &format!("/api/conversations/{chat}/preferences"),
            &owner.token,
        )
        .await;
    assert!(
        preferences["muted_until"].is_null(),
        "the chat itself is not muted: {preferences}"
    );
    assert_eq!(unread(&server, &chat, &owner, "Beta").await, (0, false));
    send_in_topic(&mut socket, "a3", Some(&alpha)).await;
    let a3 = broadcast_of(&mut socket, "a3").await;
    assert_eq!(
        unread(&server, &chat, &owner, "Alpha").await,
        (1, true),
        "muted still counts"
    );
    let owner_id = Uuid::parse_str(&owner.id).unwrap();
    let a3_id = Uuid::parse_str(a3["message_id"].as_str().unwrap()).unwrap();
    assert!(
        state.message_topic_muted(owner_id, a3_id).await.unwrap(),
        "push is silenced"
    );
    let b1_id = Uuid::parse_str(sent[2]["message_id"].as_str().unwrap()).unwrap();
    assert!(!state.message_topic_muted(owner_id, b1_id).await.unwrap());
    // A mute with an expiry in the past is no mute.
    server
        .put(
            &format!("/api/chats/{chat}/topics/{alpha}/notifications"),
            &owner.token,
            json!({ "muted": true, "muted_until": "2020-01-01T00:00:00Z" }),
        )
        .await;
    assert_eq!(unread(&server, &chat, &owner, "Alpha").await, (1, false));
    assert!(!state.message_topic_muted(owner_id, a3_id).await.unwrap());
    // Muting the chat does not mute a topic.
    let until = (chrono::Utc::now() + chrono::Duration::hours(1)).to_rfc3339();
    let (status, _) = server
        .call(
            Method::PATCH,
            &format!("/api/conversations/{chat}/preferences"),
            &owner.token,
            Some(json!({ "muted_until": until })),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(unread(&server, &chat, &owner, "Beta").await, (0, false));
}

async fn deletion_scenario(state: Arc<AppState>) {
    let server = serve(state.clone()).await;
    let owner = server.register("tg204-deleter").await;
    let member = server.register("tg204-poster").await;
    let chat = server.create_group(&owner, "tg204-delete").await;
    server.join(&chat, &member).await;
    enable_forum(&server, &chat, &owner).await;
    let doomed = new_topic(&server, &chat, &owner, "Doomed").await;
    let mut socket = server.socket(&chat, &owner).await;
    send_in_topic(&mut socket, "kept in General", None).await;
    broadcast_of(&mut socket, "kept in General").await;
    send_in_topic(&mut socket, "goes away", Some(&doomed)).await;
    broadcast_of(&mut socket, "goes away").await;
    let (_, lonely) = upload(&server, &chat, &member.token, Some(&doomed)).await;
    let (_, shared) = upload(&server, &chat, &member.token, Some(&doomed)).await;
    // A copy forwarded elsewhere keeps its file alive; the other file loses its last reference.
    let elsewhere = server.create_group(&member, "tg204-elsewhere").await;
    let (_, forwarded) = server
        .call(
            Method::POST,
            "/api/messages/forward",
            &member.token,
            Some(json!({ "message_ids": [shared["id"]], "target_room_ids": [elsewhere] })),
        )
        .await;
    assert!(
        forwarded[0]["forwarded_message_id"].is_string(),
        "{forwarded}"
    );
    // Same bytes, one content-addressed object: the forwarded copy references the same file.
    let lonely_attachment = lonely["attachment"]["id"].as_str().unwrap().to_string();

    let path = format!("/api/chats/{chat}/topics/{doomed}");
    let (status, _) = server
        .call(Method::DELETE, &path, &member.token, None)
        .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "deleting is for topic admins"
    );
    let (status, _) = server.call(Method::DELETE, &path, &owner.token, None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let frame = next_frame(&mut socket, "topic_updated").await;
    assert_eq!(
        (
            frame["topic"]["id"].as_str(),
            frame["topic"]["deleted"].as_bool()
        ),
        (Some(doomed.as_str()), Some(true))
    );

    let history = chat_history(&server, &chat, &owner.token).await;
    assert_eq!(
        contents(&history),
        ["kept in General"],
        "the topic's messages are deleted"
    );
    let (status, _) = server.get(&path, &owner.token).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(
        upload(&server, &chat, &owner.token, Some(&doomed)).await.0,
        StatusCode::NOT_FOUND
    );
    // Both uploads carried identical bytes, so they share one stored object with the forward:
    // still referenced, nothing is orphaned.
    assert!(!orphaned(&state, &lonely_attachment).await);
    let forwarded_history = chat_history(&server, &elsewhere, &member.token).await;
    assert_eq!(forwarded_history.len(), 1, "the forwarded copy survives");
    // Deleting the last reference orphans the object.
    let last = new_topic(&server, &chat, &owner, "Last").await;
    let (_, unique) = unique_upload(&server, &chat, &member.token, &last).await;
    let attachment = unique["attachment"]["id"].as_str().unwrap().to_string();
    assert!(!orphaned(&state, &attachment).await);
    let (status, _) = server
        .call(
            Method::DELETE,
            &format!("/api/chats/{chat}/topics/{last}"),
            &owner.token,
            None,
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert!(
        orphaned(&state, &attachment).await,
        "the recompute marked the file orphaned"
    );
}

/// An upload whose bytes nothing else shares.
async fn unique_upload(
    server: &Server,
    chat: &str,
    token: &str,
    topic: &str,
) -> (StatusCode, Value) {
    let part = reqwest::multipart::Part::bytes(Uuid::new_v4().as_bytes().to_vec())
        .file_name("unique.bin")
        .mime_str("application/octet-stream")
        .unwrap();
    let form = reqwest::multipart::Form::new()
        .part("file", part)
        .text("topic_id", topic.to_string());
    let response = server
        .client
        .post(format!("{}/api/chats/{chat}/attachments", server.base))
        .bearer_auth(token)
        .multipart(form)
        .send()
        .await
        .unwrap();
    let status = response.status();
    assert_eq!(status, StatusCode::CREATED);
    (status, response.json().await.unwrap())
}

#[tokio::test]
async fn topic_unread_and_mute_on_sqlite() {
    unread_and_mute_scenario(Arc::new(AppState::new().await.unwrap())).await;
}

#[tokio::test]
async fn topic_unread_and_mute_on_postgres() {
    with_postgres(
        "topic_unread_and_mute_on_postgres",
        unread_and_mute_scenario,
    )
    .await;
}

#[tokio::test]
async fn topic_deletion_on_sqlite() {
    deletion_scenario(Arc::new(AppState::new().await.unwrap())).await;
}

#[tokio::test]
async fn topic_deletion_on_postgres() {
    with_postgres("topic_deletion_on_postgres", deletion_scenario).await;
}
