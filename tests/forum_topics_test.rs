//! TG-204: forum topics end to end — enabling the forum, General, creating and editing
//! topics, posting into topics on every send path, and closed topics refusing non-admins on
//! every path while staying readable. SQLite always, PostgreSQL when configured.

use std::sync::Arc;

use chat_room::state::AppState;
use reqwest::{Method, StatusCode};
use serde_json::json;

mod chat_admin_support;
mod forum_support;

use chat_admin_support::{next_frame, serve, with_postgres, Account, Server};
use forum_support::creator::creator_rights_checks;
use forum_support::*;

async fn lifecycle_scenario(state: Arc<AppState>) {
    let server = serve(state).await;
    let owner = server.register("tg204-owner").await;
    let member = server.register("tg204-member").await;
    let chat = server.create_group(&owner, "tg204-forum").await;
    server.join(&chat, &member).await;
    let mut owner_socket = server.socket(&chat, &owner).await;
    send_in_topic(&mut owner_socket, "before the forum", None).await;
    broadcast_of(&mut owner_socket, "before the forum").await;

    // Not a forum yet: topics are 409, a topic id on a send is refused.
    let (status, _) = server
        .get(&format!("/api/chats/{chat}/topics"), &owner.token)
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(
        poll(
            &server,
            &chat,
            &member.token,
            Some(&uuid::Uuid::new_v4().to_string())
        )
        .await,
        StatusCode::BAD_REQUEST
    );
    // Only chat.info turns it on; turning it on upgrades the group.
    let (status, _) = server
        .put(
            &format!("/api/chats/{chat}/forum"),
            &member.token,
            json!({ "enabled": true }),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let descriptor = enable_forum(&server, &chat, &owner).await;
    assert_eq!(descriptor["is_forum"], true);
    assert_eq!(descriptor["chat_type"], "supergroup");
    assert_eq!(
        next_frame(&mut owner_socket, "chat_updated").await["chat"]["is_forum"],
        true
    );

    // Existing history belongs to General, which is first and cannot be deleted.
    let list = topics(&server, &chat, &member).await;
    assert_eq!(
        (list["can_create"].clone(), list["can_manage"].clone()),
        (json!(false), json!(false))
    );
    let general_topic = general(&server, &chat, &member).await;
    assert_eq!(general_topic["title"], "General");
    assert_eq!(general_topic["last_message"]["content"], "before the forum");
    let general_id = general_topic["id"].as_str().unwrap().to_string();
    let (status, _) = server
        .call(
            Method::DELETE,
            &format!("/api/chats/{chat}/topics/{general_id}"),
            &owner.token,
            None,
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT);

    // Creating needs chat.topics and validates the input.
    let (status, _) = create_topic(&server, &chat, &member, json!({ "title": "nope" })).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    for bad in [
        json!({ "title": "  " }),
        json!({ "title": "x", "icon_color": 0x123456 }),
    ] {
        let (status, _) = create_topic(&server, &chat, &owner, bad).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }
    let (status, created) = create_topic(
        &server,
        &chat,
        &owner,
        json!({ "title": " Ideas ", "icon_emoji": "💡", "icon_color": RED }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(
        (created["title"].as_str(), created["icon_color"].as_i64()),
        (Some("Ideas"), Some(RED))
    );
    let ideas = created["id"].as_str().unwrap().to_string();
    let frame = next_frame(&mut owner_socket, "topic_updated").await;
    assert_eq!(frame["topic"]["id"], ideas.as_str());
    assert_eq!(frame["topic"]["icon_emoji"], "💡");

    // Posting into the topic on each path; the frame and every page carry the topic.
    let mut member_socket = server.socket(&chat, &member).await;
    send_in_topic(&mut member_socket, "idea one", Some(&ideas)).await;
    assert_eq!(
        broadcast_of(&mut owner_socket, "idea one").await["topic_id"],
        ideas.as_str()
    );
    assert_eq!(
        upload(&server, &chat, &member.token, Some(&ideas)).await.0,
        StatusCode::CREATED
    );
    assert_eq!(
        chunked_upload(&server, &chat, &member.token, Some(&ideas)).await,
        StatusCode::CREATED
    );
    assert_eq!(
        poll(&server, &chat, &member.token, Some(&ideas)).await,
        StatusCode::CREATED
    );
    send_in_topic(&mut member_socket, "general chatter", Some(&general_id)).await;
    let general_frame = broadcast_of(&mut owner_socket, "general chatter").await;
    assert!(
        general_frame.get("topic_id").is_none(),
        "General is stored as NULL"
    );
    let idea_page = topic_history(&server, &chat, &ideas, &owner.token).await;
    assert_eq!(
        contents(&idea_page),
        ["idea one", "a file", "chunked", "Lunch?"]
    );
    assert!(idea_page
        .iter()
        .all(|message| message["topic_id"] == ideas.as_str()));
    let general_page = topic_history(&server, &chat, &general_id, &owner.token).await;
    assert_eq!(
        contents(&general_page),
        ["before the forum", "general chatter"]
    );
    // Same ordering and inclusive `before` as `/messages`.
    let before = idea_page[2]["id"].as_str().unwrap();
    let (_, older) = server
        .get(
            &format!("/api/chats/{chat}/topics/{ideas}/messages?before={before}&limit=2"),
            &owner.token,
        )
        .await;
    assert_eq!(contents(older.as_array().unwrap()), ["a file", "chunked"]);
    assert_eq!(chat_history(&server, &chat, &owner.token).await.len(), 6);
    let (status, window) = server
        .get(
            &format!("/api/chats/{chat}/topics/{ideas}/messages/{before}/context?limit=10"),
            &owner.token,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(contents(window.as_array().unwrap()).len(), 4);
    let other = general_page[0]["id"].as_str().unwrap();
    let (status, _) = server
        .get(
            &format!("/api/chats/{chat}/topics/{ideas}/messages/{other}/context"),
            &owner.token,
        )
        .await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "a General message is not in Ideas"
    );

    closed_topic_checks(&server, &chat, &owner, &member, &ideas, &general_id).await;
    creator_rights_checks(&server, &chat, &owner, &member, &ideas).await;
}

/// A closed topic refuses non-admins on every path, stays readable, and admins still post.
async fn closed_topic_checks(
    server: &Server,
    chat: &str,
    owner: &Account,
    member: &Account,
    ideas: &str,
    general_id: &str,
) {
    let (status, _) = patch_topic(server, chat, ideas, member, json!({ "is_closed": true })).await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "not the creator, not an admin"
    );
    let (status, closed) =
        patch_topic(server, chat, ideas, owner, json!({ "is_closed": true })).await;
    assert_eq!(
        (status, closed["is_closed"].clone()),
        (StatusCode::OK, json!(true))
    );

    let mut member_socket = server.socket(chat, member).await;
    let mut owner_socket = server.socket(chat, owner).await;
    send_in_topic(&mut member_socket, "sneaky", Some(ideas)).await;
    send_in_topic(&mut member_socket, "after sneaky", None).await;
    broadcast_of(&mut owner_socket, "after sneaky").await;
    assert_eq!(
        upload(server, chat, &member.token, Some(ideas)).await.0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        chunked_upload(server, chat, &member.token, Some(ideas)).await,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        poll(server, chat, &member.token, Some(ideas)).await,
        StatusCode::FORBIDDEN
    );
    let (status, _) = server
        .call(
            Method::POST,
            &format!("/api/chats/{chat}/sticker-messages"),
            &member.token,
            Some(json!({ "sticker_id": uuid::Uuid::new_v4(), "topic_id": ideas })),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    // Read side: the member still reads the closed topic, which kept none of the attempts.
    let page = topic_history(server, chat, ideas, &member.token).await;
    assert_eq!(contents(&page), ["idea one", "a file", "chunked", "Lunch?"]);
    // An administrator still posts.
    send_in_topic(&mut owner_socket, "admin note", Some(ideas)).await;
    broadcast_of(&mut owner_socket, "admin note").await;
    // Closing General closes the chat's default destination too, forwards included.
    patch_topic(
        server,
        chat,
        general_id,
        owner,
        json!({ "is_closed": true }),
    )
    .await;
    assert_eq!(
        upload(server, chat, &member.token, None).await.0,
        StatusCode::FORBIDDEN
    );
    let source = server.create_group(member, "tg204-source").await;
    let mut source_socket = server.socket(&source, member).await;
    send_in_topic(&mut source_socket, "to forward", None).await;
    let message_id = broadcast_of(&mut source_socket, "to forward").await["message_id"].clone();
    let (_, results) = server
        .call(
            Method::POST,
            "/api/messages/forward",
            &member.token,
            Some(json!({ "message_ids": [message_id], "target_room_ids": [chat] })),
        )
        .await;
    assert_eq!(results[0]["skipped_reason"], "the target topic is closed");
    // Reopening lets members post again.
    patch_topic(
        server,
        chat,
        general_id,
        owner,
        json!({ "is_closed": false }),
    )
    .await;
    patch_topic(server, chat, ideas, owner, json!({ "is_closed": false })).await;
    assert_eq!(
        poll(server, chat, &member.token, Some(ideas)).await,
        StatusCode::CREATED
    );
}

#[tokio::test]
async fn forum_topic_lifecycle_on_sqlite() {
    lifecycle_scenario(Arc::new(AppState::new().await.unwrap())).await;
}

#[tokio::test]
async fn forum_topic_lifecycle_on_postgres() {
    with_postgres("forum_topic_lifecycle_on_postgres", lifecycle_scenario).await;
}
