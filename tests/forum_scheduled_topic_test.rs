//! TG-204 × TG-404: a scheduled message is scheduled into a topic, obeys the closed-topic gate
//! when scheduled, and is delivered into that topic. (The voice and GIF send paths call the
//! same `resolve_post_topic` gate before inserting.)

use std::sync::Arc;

use chat_room::state::AppState;
use chrono::{Duration, Utc};
use reqwest::{Method, StatusCode};
use serde_json::json;

mod chat_admin_support;
mod forum_support;

use chat_admin_support::serve;
use forum_support::*;

#[tokio::test]
async fn scheduled_messages_land_in_their_topic_and_respect_closed_topics() {
    let server = serve(Arc::new(AppState::new().await.unwrap())).await;
    let owner = server.register("tg204-sched-owner").await;
    let member = server.register("tg204-sched-member").await;
    let chat = server.create_group(&owner, "tg204-sched").await;
    server.join(&chat, &member).await;
    enable_forum(&server, &chat, &owner).await;
    let ideas = new_topic(&server, &chat, &owner, "Ideas").await;
    let later = (Utc::now() + Duration::hours(1)).to_rfc3339();
    let path = format!("/api/chats/{chat}/scheduled-messages");

    let (status, scheduled) = server
        .call(
            Method::POST,
            &path,
            &member.token,
            Some(json!({ "content": "into ideas", "scheduled_at": later, "topic_id": ideas })),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{scheduled}");
    assert_eq!(scheduled["topic_id"], ideas.as_str());

    let (status, delivered) = server
        .call(
            Method::POST,
            &format!("{path}/{}/send-now", scheduled["id"].as_str().unwrap()),
            &member.token,
            None,
        )
        .await;
    assert!(status.is_success(), "send now: {status} {delivered}");
    let page = topic_history(&server, &chat, &ideas, &member.token).await;
    assert!(contents(&page).contains(&"into ideas"), "{page:?}");

    let (status, _) =
        patch_topic(&server, &chat, &ideas, &owner, json!({ "is_closed": true })).await;
    assert_eq!(status, StatusCode::OK);
    let (status, refused) = server
        .call(
            Method::POST,
            &path,
            &member.token,
            Some(json!({ "content": "too late", "scheduled_at": later, "topic_id": ideas })),
        )
        .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "closed topic accepted: {refused}"
    );
}
