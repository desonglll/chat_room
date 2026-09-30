//! TG-404: scheduled messages over HTTP — invisibility before delivery (history, search,
//! unread counts, chat-list previews, other members), author-only access, edit / reschedule /
//! cancel, validation, and exactly-once «send now».

mod poll_support;
mod scheduled_support;

use poll_support::{create_chat, join, register, serve_memory};
use reqwest::StatusCode;
use scheduled_support::{
    conversation, delete, get_json, history_count, in_seconds, list, patch, schedule, schedule_ok,
    send_now,
};
use serde_json::json;

const SECRET: &str = "tg404zebracanary";

#[tokio::test]
async fn a_scheduled_message_is_invisible_to_everyone_until_it_is_delivered() {
    let server = serve_memory().await;
    let base = &server.base;
    let alice = register(base, "tg404-inv-alice").await;
    let bob = register(base, "tg404-inv-bob").await;
    let chat = create_chat(base, &alice, "tg404 invisible").await;
    join(base, &bob, chat).await;

    let id = schedule_ok(base, &alice, chat, &format!("plan {SECRET}"), 3600, false).await;

    // Only the author's list shows it.
    let (status, own) = list(base, &alice, chat).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(own.as_array().unwrap().len(), 1);
    assert_eq!(own[0]["id"], id.to_string());
    assert_eq!(own[0]["chat_id"], chat.to_string());
    let (status, others) = list(base, &bob, chat).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        others.as_array().unwrap().is_empty(),
        "bob sees no one else's"
    );

    for account in [&alice, &bob] {
        assert_eq!(history_count(base, account, chat, id).await, 0);
        let history = get_json(base, account, &format!("/api/chats/{chat}/messages")).await;
        assert!(!history.to_string().contains(SECRET), "history leaks it");
        let search = get_json(
            base,
            account,
            &format!("/api/chats/{chat}/messages/search?q={SECRET}"),
        )
        .await;
        assert!(!search.to_string().contains(SECRET), "chat search leaks it");
        let global = get_json(base, account, &format!("/api/messages/search?q={SECRET}")).await;
        assert!(
            !global.to_string().contains(SECRET),
            "global search leaks it"
        );
        let row = conversation(base, account, chat).await;
        assert!(
            !row.to_string().contains(SECRET),
            "chat-list preview leaks it"
        );
    }
    assert_eq!(conversation(base, &bob, chat).await["unread_count"], 0);
    let notifications = get_json(base, &bob, "/api/notifications").await;
    assert!(!notifications.to_string().contains(SECRET));

    // Delivered: now it is an ordinary message under the same id, and the list is empty.
    let (status, message) = send_now(base, &alice, chat, id).await;
    assert_eq!(status, StatusCode::OK, "send now: {message}");
    assert_eq!(message["id"], id.to_string());
    assert!(message.get("silent").is_none(), "not silent: field omitted");
    assert_eq!(history_count(base, &bob, chat, id).await, 1);
    assert_eq!(conversation(base, &bob, chat).await["unread_count"], 1);
    let search = get_json(
        base,
        &bob,
        &format!("/api/chats/{chat}/messages/search?q={SECRET}"),
    )
    .await;
    assert!(
        search.to_string().contains(SECRET),
        "delivered text is searchable"
    );
    let (_, own) = list(base, &alice, chat).await;
    assert!(own.as_array().unwrap().is_empty());
}

#[tokio::test]
async fn only_the_author_can_read_edit_cancel_or_send_a_scheduled_message() {
    let server = serve_memory().await;
    let base = &server.base;
    let alice = register(base, "tg404-auth-alice").await;
    let bob = register(base, "tg404-auth-bob").await;
    let mallory = register(base, "tg404-auth-mallory").await;
    let chat = create_chat(base, &alice, "tg404 auth").await;
    let other_chat = create_chat(base, &alice, "tg404 auth other").await;
    join(base, &bob, chat).await;
    let id = schedule_ok(base, &alice, chat, "mine", 3600, false).await;

    // A member who is not the author: every per-message route answers 404.
    let (status, _) = patch(base, &bob, chat, id, json!({ "content": "hijack" })).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(delete(base, &bob, chat, id).await, StatusCode::NOT_FOUND);
    assert_eq!(
        send_now(base, &bob, chat, id).await.0,
        StatusCode::NOT_FOUND
    );

    // A non-member cannot list, create, or touch anything.
    assert_eq!(list(base, &mallory, chat).await.0, StatusCode::NOT_FOUND);
    let (status, _) = schedule(
        base,
        &mallory,
        chat,
        json!({ "content": "x", "scheduled_at": in_seconds(60) }),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(
        delete(base, &mallory, chat, id).await,
        StatusCode::NOT_FOUND
    );

    // The right id under the wrong chat is not found either.
    assert_eq!(
        send_now(base, &alice, other_chat, id).await.0,
        StatusCode::NOT_FOUND
    );

    let (_, own) = list(base, &alice, chat).await;
    assert_eq!(own[0]["content"], "mine", "nothing above changed it");
}

#[tokio::test]
async fn the_author_edits_reschedules_toggles_silent_and_cancels() {
    let server = serve_memory().await;
    let base = &server.base;
    let alice = register(base, "tg404-edit-alice").await;
    let chat = create_chat(base, &alice, "tg404 edit").await;
    let id = schedule_ok(base, &alice, chat, "draft", 3600, false).await;

    let later = in_seconds(7200);
    let (status, updated) = patch(
        base,
        &alice,
        chat,
        id,
        json!({ "content": "  final  ", "scheduled_at": later, "silent": true }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    assert_eq!(updated["content"], "final");
    assert_eq!(updated["silent"], true);
    let moved: chrono::DateTime<chrono::Utc> =
        updated["scheduled_at"].as_str().unwrap().parse().unwrap();
    let wanted: chrono::DateTime<chrono::Utc> = later.parse().unwrap();
    assert!((moved - wanted).num_milliseconds().abs() < 1000);

    // A partial update keeps the other fields.
    let (status, updated) = patch(base, &alice, chat, id, json!({ "silent": false })).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(updated["content"], "final");
    assert_eq!(updated["silent"], false);

    let (status, _) = patch(
        base,
        &alice,
        chat,
        id,
        json!({ "scheduled_at": in_seconds(-5) }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "past date");
    let (status, _) = patch(base, &alice, chat, id, json!({ "content": "   " })).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "empty text");

    assert_eq!(delete(base, &alice, chat, id).await, StatusCode::NO_CONTENT);
    assert_eq!(delete(base, &alice, chat, id).await, StatusCode::NOT_FOUND);
    assert_eq!(
        send_now(base, &alice, chat, id).await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        history_count(base, &alice, chat, id).await,
        0,
        "cancelled never sends"
    );
}

#[tokio::test]
async fn scheduling_rejects_bad_dates_and_empty_text() {
    let server = serve_memory().await;
    let base = &server.base;
    let alice = register(base, "tg404-valid-alice").await;
    let chat = create_chat(base, &alice, "tg404 validation").await;
    for (body, why) in [
        (
            json!({ "content": "x", "scheduled_at": in_seconds(-1) }),
            "past",
        ),
        (
            json!({ "content": "x", "scheduled_at": in_seconds(367 * 86_400) }),
            "over a year",
        ),
        (
            json!({ "content": " ", "scheduled_at": in_seconds(60) }),
            "empty",
        ),
    ] {
        let (status, _) = schedule(base, &alice, chat, body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{why}");
    }
    let (_, own) = list(base, &alice, chat).await;
    assert!(own.as_array().unwrap().is_empty());
}

#[tokio::test]
async fn concurrent_send_now_delivers_exactly_once() {
    let server = serve_memory().await;
    let base = server.base.clone();
    let alice = register(&base, "tg404-once-alice").await;
    let chat = create_chat(&base, &alice, "tg404 once").await;
    let id = schedule_ok(&base, &alice, chat, "only once", 3600, false).await;

    let attempts = (0..6).map(|_| {
        let (base, alice) = (base.clone(), alice.clone());
        tokio::spawn(async move { send_now(&base, &alice, chat, id).await.0 })
    });
    let mut statuses = Vec::new();
    for attempt in attempts {
        statuses.push(attempt.await.unwrap());
    }
    let delivered = statuses.iter().filter(|s| **s == StatusCode::OK).count();
    assert_eq!(delivered, 1, "{statuses:?}");
    assert!(statuses
        .iter()
        .all(|s| *s == StatusCode::OK || *s == StatusCode::NOT_FOUND));
    assert_eq!(history_count(&base, &alice, chat, id).await, 1);
}

#[tokio::test]
async fn a_member_who_lost_the_chat_loses_the_scheduled_message() {
    let server = serve_memory().await;
    let base = &server.base;
    let alice = register(base, "tg404-left-alice").await;
    let bob = register(base, "tg404-left-bob").await;
    let chat = create_chat(base, &alice, "tg404 left").await;
    join(base, &bob, chat).await;
    let id = schedule_ok(base, &bob, chat, "after I left", 2, false).await;
    let (status, _) = poll_support::call(
        reqwest::Method::DELETE,
        format!("{base}/api/chats/{chat}/members/me"),
        &bob.token,
        None,
    )
    .await;
    assert!(status.is_success(), "leave: {status}");
    tokio::time::sleep(std::time::Duration::from_secs(4)).await;
    assert_eq!(history_count(base, &alice, chat, id).await, 0);
    let left: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM scheduled_messages WHERE id = $1")
        .bind(id)
        .fetch_one(server.state.pool())
        .await
        .unwrap();
    assert_eq!(
        left, 0,
        "the undeliverable row is discarded, not retried forever"
    );
}
