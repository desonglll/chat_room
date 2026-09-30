//! TG-202 acceptance: channel creation, silent lightweight subscriptions, the subscriber
//! count, `message.post` on every send path (write time) and in the permission view and
//! history gate (read time), signatures, and the moderation keys TG-201 left unenforced.

mod channel_support;
mod poll_support;

use std::time::Duration;

use channel_support::*;
use reqwest::{Method, StatusCode};
use serde_json::{json, Value};
use uuid::Uuid;

#[tokio::test]
async fn a_channel_is_created_and_other_types_are_refused() {
    let server = serve_memory().await;
    let base = &server.base;
    let owner = register(base, "ch-create-owner").await;
    let channel = create_channel(base, &owner, "news", true).await;
    assert_eq!(channel["chat_type"], "channel");
    assert_eq!(channel["signatures_enabled"], true);
    assert_eq!(channel["membership_role"], "owner");
    assert_eq!(
        get_chat(base, &owner, id_of(&channel)).await["member_count"],
        1
    );

    for chat_type in ["supergroup", "private", "megagroup"] {
        let (status, _) = call(
            Method::POST,
            format!("{base}/api/chats"),
            &owner.token,
            Some(json!({ "title": format!("t-{chat_type}"), "chat_type": chat_type })),
        )
        .await;
        assert!(status.is_client_error(), "{chat_type}: {status}");
    }
    // A group ignores the signature switch.
    let (status, group) = call(
        Method::POST,
        format!("{base}/api/chats"),
        &owner.token,
        Some(json!({ "title": "plain", "signatures_enabled": true })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(group["chat_type"], "group");
    assert_eq!(group["signatures_enabled"], false);
}

#[tokio::test]
async fn subscribing_is_silent_and_moves_the_subscriber_count() {
    let server = serve_memory().await;
    let base = &server.base;
    let owner = register(base, "ch-sub-owner").await;
    let bob = register(base, "ch-sub-bob").await;
    let carol = register(base, "ch-sub-carol").await;
    let channel = id_of(&create_channel(base, &owner, "silent", false).await);
    let mut owner_socket = open_socket(base, channel, &owner).await;

    let (status, membership) = subscribe(base, &bob, channel).await;
    assert_eq!(status, StatusCode::OK, "{membership}");
    assert_eq!(membership["status"], "active");
    assert_eq!(membership["role"], "member");
    assert_eq!(
        subscribe(base, &bob, channel).await.0,
        StatusCode::OK,
        "idempotent"
    );
    assert_eq!(subscribe(base, &carol, channel).await.0, StatusCode::OK);
    assert_eq!(get_chat(base, &owner, channel).await["member_count"], 3);

    // A subscriber's socket: staff-only participants, no presence/join frame for anyone.
    let url = format!("{}/ws/{channel}", base.replacen("http://", "ws://", 1));
    let (mut bob_socket, _) = tokio_tungstenite::connect_async(url).await.unwrap();
    send_frame(
        &mut bob_socket,
        json!({ "type": "join", "token": bob.token }),
    )
    .await;
    let auth_ok = next_type(&mut bob_socket, "auth_ok").await;
    let participants: Vec<&str> = auth_ok["participants"]
        .as_array()
        .unwrap()
        .iter()
        .map(|member| member["username"].as_str().unwrap())
        .collect();
    assert_eq!(participants, vec!["ch-sub-owner"]);
    assert!(auth_ok["read_receipts"].as_array().unwrap().is_empty());
    next_type(&mut bob_socket, "history_complete").await;

    assert_eq!(
        unsubscribe(base, &carol, channel).await,
        StatusCode::NO_CONTENT
    );
    assert_eq!(get_chat(base, &owner, channel).await["member_count"], 2);
    assert_eq!(
        unsubscribe(base, &owner, channel).await,
        StatusCode::CONFLICT
    );

    for kind in ["system", "presence", "user_status"] {
        let frames = drain_type(&mut owner_socket, kind, Duration::from_millis(300)).await;
        assert!(frames.is_empty(), "a channel broadcast {kind}: {frames:?}");
    }

    // Subscribing is for channels only.
    let group = create_chat(base, &owner, "not-a-channel").await;
    assert_eq!(subscribe(base, &bob, group).await.0, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn only_post_holders_publish_on_every_send_path() {
    let server = serve_memory().await;
    let base = &server.base;
    let owner = register(base, "ch-post-owner").await;
    let bob = register(base, "ch-post-bob").await;
    let editor = register(base, "ch-post-editor").await;
    let moderator = register(base, "ch-post-moderator").await;
    let channel = id_of(&create_channel(base, &owner, "posts", false).await);
    for account in [&bob, &editor, &moderator] {
        assert_eq!(subscribe(base, account, channel).await.0, StatusCode::OK);
    }
    appoint(
        base,
        &owner,
        channel,
        &editor,
        &["message.post", "message.edit_any"],
    )
    .await;
    // An administrator without `message.post` cannot publish either.
    appoint(base, &owner, channel, &moderator, &["message.delete_any"]).await;

    // Read time: the permission view tells each viewer whether they may post.
    let subscriber = my_permissions(base, &bob, channel).await;
    assert!(
        subscriber.is_empty(),
        "a subscriber holds nothing: {subscriber:?}"
    );
    let editor_keys = my_permissions(base, &editor, channel).await;
    for key in [
        "message.post",
        "message.send",
        "message.send_media",
        "message.send_poll",
    ] {
        assert!(
            editor_keys.contains(&key.to_string()),
            "{key}: {editor_keys:?}"
        );
    }
    let moderator_keys = my_permissions(base, &moderator, channel).await;
    assert!(!moderator_keys.contains(&"message.post".to_string()));
    assert!(!moderator_keys.contains(&"message.send_media".to_string()));

    // A subscriber's group elsewhere, as a forward source.
    let source = create_chat(base, &bob, "bob-source").await;
    let mut source_socket = open_socket(base, source, &bob).await;
    say(&mut source_socket, "forward me").await;
    let forwardable = next_type(&mut source_socket, "broadcast").await["message_id"]
        .as_str()
        .unwrap()
        .to_string();

    let mut owner_socket = open_socket(base, channel, &owner).await;
    let mut bob_socket = open_socket(base, channel, &bob).await;
    let mut editor_socket = open_socket(base, channel, &editor).await;
    let mut moderator_socket = open_socket(base, channel, &moderator).await;

    // WebSocket send.
    say(&mut bob_socket, "subscriber post").await;
    say(&mut moderator_socket, "moderator post").await;
    // REST sends: attachment, chunked upload, sticker, poll, pin, forward, favorite forward.
    for account in [&bob, &moderator] {
        assert_eq!(
            upload_status(base, account, channel).await,
            StatusCode::FORBIDDEN
        );
        let (status, _) = call(
            Method::POST,
            format!("{base}/api/chats/{channel}/attachments/uploads"),
            &account.token,
            Some(json!({ "file_name": "a.txt", "mime_type": "text/plain", "size_bytes": 10 })),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN, "chunked upload");
        let (status, _) = call(
            Method::POST,
            format!("{base}/api/chats/{channel}/sticker-messages"),
            &account.token,
            Some(json!({ "sticker_id": Uuid::new_v4() })),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN, "sticker");
        let (status, _) = create_poll(
            base,
            account,
            channel,
            json!({ "question": "Q?", "options": ["a", "b"] }),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN, "poll");
    }
    let (status, body) = call(
        Method::POST,
        format!("{base}/api/messages/forward"),
        &bob.token,
        Some(json!({ "message_ids": [forwardable], "target_room_ids": [channel] })),
    )
    .await;
    assert!(
        status == StatusCode::FORBIDDEN
            || body.as_array().is_some_and(|results| results
                .iter()
                .all(|result| result["forwarded_message_id"].is_null())),
        "forward into a channel: {status} {body}"
    );
    let (_, favorite) = call(
        Method::POST,
        format!("{base}/api/favorites"),
        &bob.token,
        Some(json!({ "title": "note", "content": "favorite body" })),
    )
    .await;
    let (status, results) = call(
        Method::POST,
        format!(
            "{base}/api/favorites/{}/forward",
            favorite["id"].as_str().unwrap()
        ),
        &bob.token,
        Some(json!({ "target_room_ids": [channel] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{results}");
    assert!(results[0]["forwarded_message_id"].is_null(), "{results}");

    // The owner and the posting administrator publish, media included (TG-201 follow-up:
    // the content kinds depend on `message.post` in a channel).
    say(&mut owner_socket, "owner post").await;
    say(&mut editor_socket, "editor post").await;
    assert_eq!(
        upload_status(base, &editor, channel).await,
        StatusCode::CREATED
    );
    let (status, poll_body) = create_poll(
        base,
        &editor,
        channel,
        json!({ "question": "Editor poll?", "options": ["a", "b"] }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{poll_body}");
    tokio::time::sleep(Duration::from_millis(300)).await;

    let published = contents(base, &owner, channel).await;
    for expected in ["owner post", "editor post", "Editor poll?"] {
        assert!(
            published.iter().any(|content| content == expected),
            "{published:?}"
        );
    }
    for refused in [
        "subscriber post",
        "moderator post",
        "forward me",
        "favorite body",
        "Q?",
    ] {
        assert!(
            !published.iter().any(|content| content == refused),
            "{refused} got through"
        );
    }
    assert_eq!(
        published.len(),
        4,
        "owner, editor, attachment, poll: {published:?}"
    );

    // Read time: the history is the subscribers'; an outsider reads nothing.
    let stranger = register(base, "ch-post-stranger").await;
    let (status, _) = call(
        Method::GET,
        format!("{base}/api/chats/{channel}/messages"),
        &stranger.token,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(contents(base, &bob, channel).await.len(), 4);
    drop((
        owner_socket,
        bob_socket,
        editor_socket,
        moderator_socket,
        source_socket,
    ));
}

#[tokio::test]
async fn edit_any_and_delete_any_are_enforced_and_subscribers_react() {
    let server = serve_memory().await;
    let base = &server.base;
    let owner = register(base, "ch-mod-owner").await;
    let bob = register(base, "ch-mod-bob").await;
    let editor = register(base, "ch-mod-editor").await;
    let channel = id_of(&create_channel(base, &owner, "moderated", false).await);
    for account in [&bob, &editor] {
        subscribe(base, account, channel).await;
    }
    appoint(
        base,
        &owner,
        channel,
        &editor,
        &["message.post", "message.edit_any", "message.delete_any"],
    )
    .await;
    let mut owner_socket = open_socket(base, channel, &owner).await;
    let mut bob_socket = open_socket(base, channel, &bob).await;
    let mut editor_socket = open_socket(base, channel, &editor).await;
    say(&mut owner_socket, "original").await;
    let post = next_type(&mut owner_socket, "broadcast").await;
    let post_id = post["message_id"].as_str().unwrap().to_string();
    // A channel post is sent in the channel's name.
    assert_eq!(post["sender"], "moderated");
    assert_eq!(post["views"], 0);

    // A subscriber can neither edit nor recall someone else's post, but may react.
    send_frame(
        &mut bob_socket,
        json!({ "type": "edit", "message_id": post_id, "content": "vandalised" }),
    )
    .await;
    send_frame(
        &mut bob_socket,
        json!({ "type": "recall", "message_id": post_id }),
    )
    .await;
    send_frame(
        &mut bob_socket,
        json!({ "type": "reaction", "message_id": post_id, "emoji": "👍", "active": true }),
    )
    .await;
    let reaction = next_type(&mut owner_socket, "reaction_changed").await;
    assert_eq!(reaction["user_id"], bob.id.to_string());
    assert!(drain_type(
        &mut owner_socket,
        "message_edited",
        Duration::from_millis(300)
    )
    .await
    .is_empty());

    // An administrator holding edit_any / delete_any edits and removes the owner's post.
    send_frame(
        &mut editor_socket,
        json!({ "type": "edit", "message_id": post_id, "content": "corrected" }),
    )
    .await;
    let edited = next_type(&mut owner_socket, "message_edited").await;
    assert_eq!(edited["content"], "corrected");
    send_frame(
        &mut editor_socket,
        json!({ "type": "recall", "message_id": post_id }),
    )
    .await;
    next_type(&mut owner_socket, "message_recalled").await;
    let history = history(base, &bob, channel).await;
    assert!(history[0]["recalled_at"].is_string(), "{history:?}");
}

#[tokio::test]
async fn group_members_cannot_touch_others_messages_but_admins_delete() {
    let server = serve_memory().await;
    let base = &server.base;
    let owner = register(base, "grp-mod-owner").await;
    let admin = register(base, "grp-mod-admin").await;
    let member = register(base, "grp-mod-member").await;
    let group = create_chat(base, &owner, "grp-moderated").await;
    join(base, &admin, group).await;
    join(base, &member, group).await;
    // Default admin rights: delete_any, not edit_any.
    appoint(base, &owner, group, &admin, &["message.delete_any"]).await;
    let mut owner_socket = open_socket(base, group, &owner).await;
    let mut admin_socket = open_socket(base, group, &admin).await;
    let mut member_socket = open_socket(base, group, &member).await;
    say(&mut owner_socket, "owner words").await;
    let message_id = next_type(&mut member_socket, "broadcast").await["message_id"].clone();

    send_frame(
        &mut member_socket,
        json!({ "type": "recall", "message_id": message_id }),
    )
    .await;
    send_frame(
        &mut admin_socket,
        json!({ "type": "edit", "message_id": message_id, "content": "no edit_any" }),
    )
    .await;
    assert!(drain_type(
        &mut owner_socket,
        "message_recalled",
        Duration::from_millis(300)
    )
    .await
    .is_empty());
    send_frame(
        &mut admin_socket,
        json!({ "type": "recall", "message_id": message_id }),
    )
    .await;
    next_type(&mut owner_socket, "message_recalled").await;
    let (status, messages) = call(
        Method::GET,
        format!("{base}/api/chats/{group}/messages"),
        &member.token,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let only: &Value = &messages[0];
    assert_eq!(only["content"], "", "recalled for everyone but the sender");
    assert!(
        only.get("views").is_none(),
        "views exist only on channel posts"
    );
}

#[tokio::test]
async fn signatures_sign_posts_while_switched_on() {
    let server = serve_memory().await;
    let base = &server.base;
    let owner = register(base, "ch-sign-owner").await;
    let bob = register(base, "ch-sign-bob").await;
    let channel = id_of(&create_channel(base, &owner, "signed", true).await);
    subscribe(base, &bob, channel).await;
    let mut owner_socket = open_socket(base, channel, &owner).await;
    say(&mut owner_socket, "signed post").await;
    let signed = next_type(&mut owner_socket, "broadcast").await;
    assert_eq!(signed["post_author"], "ch-sign-owner");

    assert_eq!(
        switch(base, channel, &bob, false).await.0,
        StatusCode::FORBIDDEN
    );
    let (status, chat) = switch(base, channel, &owner, false).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(chat["signatures_enabled"], false);
    next_type(&mut owner_socket, "chat_updated").await;
    say(&mut owner_socket, "unsigned post").await;
    let unsigned = next_type(&mut owner_socket, "broadcast").await;
    assert!(unsigned.get("post_author").is_none(), "{unsigned}");

    // History carries the frozen signature.
    let history = history(base, &bob, channel).await;
    let signed_row = history
        .iter()
        .find(|message| message["content"] == "signed post")
        .unwrap();
    assert_eq!(signed_row["post_author"], "ch-sign-owner");
    assert_eq!(signed_row["sender"], "signed");
}

async fn switch(
    base: &str,
    channel: Uuid,
    account: &Account,
    enabled: bool,
) -> (StatusCode, Value) {
    call(
        Method::PATCH,
        format!("{base}/api/chats/{channel}/channel"),
        &account.token,
        Some(json!({ "signatures_enabled": enabled })),
    )
    .await
}
