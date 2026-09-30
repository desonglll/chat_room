//! The creator-rights part of `forum_topics_test.rs`, split out for the file-size gate.

use reqwest::StatusCode;
use serde_json::json;

use crate::chat_admin_support::{Account, Server};
use super::{general, new_topic, patch_topic, poll, topics, BLUE};

pub async fn grant_topic_creation(server: &Server, chat: &str, owner: &Account) {
    let (status, _) = server
        .put(
            &format!("/api/chats/{chat}/default-permissions"),
            &owner.token,
            json!({ "permissions": ["message.send", "message.send_media", "message.send_poll",
                "chat.topics"] }),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
}

pub /// A member allowed to create topics edits and closes their own, nobody else's, and never
/// pins or hides.
async fn creator_rights_checks(
    server: &Server,
    chat: &str,
    owner: &Account,
    member: &Account,
    ideas: &str,
) {
    grant_topic_creation(server, chat, owner).await;
    assert_eq!(topics(server, chat, member).await["can_create"], true);
    let own = new_topic(server, chat, member, "Member topic").await;
    let (status, renamed) = patch_topic(
        server,
        chat,
        &own,
        member,
        json!({ "title": "Renamed", "icon_color": BLUE, "is_closed": true }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        (renamed["title"].as_str(), renamed["can_edit"].as_bool()),
        (Some("Renamed"), Some(true))
    );
    for body in [json!({ "is_pinned": true }), json!({ "is_hidden": true })] {
        let (status, _) = patch_topic(server, chat, &own, member, body).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
    }
    let (status, _) = patch_topic(server, chat, ideas, member, json!({ "title": "mine" })).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    // A member with chat.topics is still not a topic admin: their own closed topic refuses them.
    assert_eq!(
        poll(server, chat, &member.token, Some(&own)).await,
        StatusCode::FORBIDDEN
    );
    // Pinning moves a topic to the top; hiding applies to General only.
    let (status, pinned) =
        patch_topic(server, chat, &own, owner, json!({ "is_pinned": true })).await;
    assert_eq!(
        (status, pinned["is_pinned"].clone()),
        (StatusCode::OK, json!(true))
    );
    assert_eq!(
        topics(server, chat, member).await["topics"][0]["id"],
        own.as_str()
    );
    let (status, _) = patch_topic(server, chat, &own, owner, json!({ "is_hidden": true })).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let general_id = general(server, chat, owner).await["id"]
        .as_str()
        .unwrap()
        .to_string();
    let (status, hidden) = patch_topic(
        server,
        chat,
        &general_id,
        owner,
        json!({ "is_hidden": true }),
    )
    .await;
    assert_eq!(
        (status, hidden["is_hidden"].clone()),
        (StatusCode::OK, json!(true))
    );
}

