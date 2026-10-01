//! TG-203 channel comments over HTTP + WebSocket, on SQLite and PostgreSQL: linking a
//! discussion group (both administrators, groups only, one channel per group), posts copied
//! into the group, commenting (members only; replies stay inside the thread), live counts,
//! comments readable after unlinking, and a recalled post taking its copy (not the comments).

mod channel_support;
mod chat_admin_support;
mod poll_support;

use std::sync::Arc;
use std::time::Duration;

use channel_support::*;
use chat_admin_support::with_postgres;
use chat_room::state::AppState;
use reqwest::{Method, StatusCode};
use serde_json::{json, Value};
use uuid::Uuid;

async fn wait_for_message(base: &str, account: &Account, chat: Uuid, content: &str) -> Value {
    for _ in 0..100 {
        if let Some(found) = history(base, account, chat)
            .await
            .into_iter()
            .find(|message| message["content"] == content)
        {
            return found;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("{content:?} never reached chat {chat}");
}

async fn link(base: &str, account: &Account, channel: Uuid, group: Value) -> StatusCode {
    call(
        Method::PUT,
        format!("{base}/api/chats/{channel}/discussion"),
        &account.token,
        Some(json!({ "chat_id": group })),
    )
    .await
    .0
}

async fn comments(base: &str, account: &Account, channel: Uuid, post: &str) -> (StatusCode, Value) {
    call(
        Method::GET,
        format!("{base}/api/chats/{channel}/posts/{post}/comments"),
        &account.token,
        None,
    )
    .await
}

async fn comment(
    base: &str,
    account: &Account,
    channel: Uuid,
    post: &str,
    body: Value,
) -> (StatusCode, Value) {
    call(
        Method::POST,
        format!("{base}/api/chats/{channel}/posts/{post}/comments"),
        &account.token,
        Some(body),
    )
    .await
}

async fn discussion_scenario(state: Arc<AppState>) {
    let server = serve(state).await;
    let base = &server.base;
    let owner = register(base, "cd-owner").await;
    let bob = register(base, "cd-bob").await;
    let channel = id_of(&create_channel(base, &owner, "news", false).await);
    let other_channel = id_of(&create_channel(base, &owner, "other news", false).await);
    let group = create_chat(base, &owner, "news chat").await;
    assert_eq!(subscribe(base, &bob, channel).await.0, StatusCode::OK);

    // Linking: administrators of both sides, a group as the target, one channel per group.
    assert_eq!(
        link(base, &bob, channel, json!(group)).await,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        link(base, &owner, channel, json!(other_channel)).await,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        link(base, &owner, channel, json!(group)).await,
        StatusCode::OK
    );
    assert_eq!(
        link(base, &owner, other_channel, json!(group)).await,
        StatusCode::CONFLICT
    );
    assert_eq!(
        get_chat(base, &owner, channel).await["linked_chat_id"],
        json!(group)
    );
    assert_eq!(
        get_chat(base, &owner, group).await["linked_chat_id"],
        json!(channel)
    );

    // A post is copied into the group, attributed to the channel.
    let mut socket = open_socket(base, channel, &owner).await;
    say(&mut socket, "first post").await;
    let post = wait_for_message(base, &owner, channel, "first post").await;
    let post_id = post["id"].as_str().unwrap().to_string();
    let copy = wait_for_message(base, &owner, group, "first post").await;
    assert!(copy["sender_id"].is_null(), "{copy}");
    assert_eq!(copy["forwarded_from"]["room_name"], "news", "{copy}");
    assert_eq!(
        wait_for_message(base, &bob, channel, "first post").await["comments"],
        0
    );

    // Reading needs only the channel; writing needs the group.
    let (status, thread) = comments(base, &bob, channel, &post_id).await;
    assert_eq!(status, StatusCode::OK, "{thread}");
    assert_eq!(thread["can_comment"], false);
    assert_eq!(thread["discussion_message_id"], copy["id"]);
    let (status, _) = comment(base, &bob, channel, &post_id, json!({ "content": "hi" })).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    join(base, &bob, group).await;
    let (status, first) =
        comment(base, &bob, channel, &post_id, json!({ "content": "nice" })).await;
    assert_eq!(status, StatusCode::CREATED, "{first}");
    let (status, _) = comment(
        base,
        &owner,
        channel,
        &post_id,
        json!({ "content": "thanks", "reply_to": first["id"] }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let (status, _) = comment(
        base,
        &owner,
        channel,
        &post_id,
        json!({ "content": "x", "reply_to": Uuid::new_v4() }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "reply outside the thread");
    let (_, thread) = comments(base, &bob, channel, &post_id).await;
    assert_eq!(thread["count"], 2);
    assert_eq!(thread["can_comment"], true);
    let texts: Vec<&str> = thread["messages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|message| message["content"].as_str().unwrap())
        .collect();
    assert_eq!(texts, ["nice", "thanks"]);
    assert_eq!(
        wait_for_message(base, &bob, channel, "first post").await["comments"],
        2
    );

    // Unlinking stops new copies but keeps old threads readable.
    assert_eq!(
        link(base, &owner, channel, Value::Null).await,
        StatusCode::OK
    );
    assert!(get_chat(base, &owner, group).await["linked_chat_id"].is_null());
    say(&mut socket, "after unlink").await;
    let later = wait_for_message(base, &owner, channel, "after unlink").await;
    assert!(later.get("comments").is_none_or(Value::is_null), "{later}");
    assert!(!contents(base, &owner, group)
        .await
        .contains(&"after unlink".to_string()));
    assert_eq!(
        comments(base, &bob, channel, &post_id).await.0,
        StatusCode::OK
    );

    // Recalling the post recalls its copy; the comments stay in the group.
    send_frame(
        &mut socket,
        json!({ "type": "recall", "message_id": post_id }),
    )
    .await;
    for _ in 0..100 {
        if comments(base, &bob, channel, &post_id).await.0 == StatusCode::NOT_FOUND {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert_eq!(
        comments(base, &bob, channel, &post_id).await.0,
        StatusCode::NOT_FOUND
    );
    let group_history = history(base, &owner, group).await;
    let copy_now = group_history
        .iter()
        .find(|message| message["id"] == copy["id"])
        .unwrap();
    assert!(!copy_now["recalled_at"].is_null(), "{copy_now}");
    for text in ["nice", "thanks"] {
        assert!(group_history
            .iter()
            .any(|message| message["content"] == text));
    }
}

#[tokio::test]
async fn sqlite_channel_comments_live_in_the_discussion_group() {
    discussion_scenario(Arc::new(AppState::new().await.unwrap())).await;
}

#[tokio::test]
async fn postgres_channel_comments_live_in_the_discussion_group() {
    with_postgres("postgres_channel_discussion", discussion_scenario).await;
}
