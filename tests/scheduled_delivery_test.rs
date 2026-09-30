//! TG-404: delivery and silence — the scheduler delivers on time through the normal message
//! path; a silent message (sent now over the WebSocket, or scheduled) produces no reply /
//! mention notification and no Web Push job, but is otherwise an ordinary message carrying
//! `silent: true`; and a scheduled message cannot trigger TG-502's unarchive before delivery.

mod poll_support;
mod scheduled_support;

use std::time::Duration;

use futures_util::SinkExt;
use poll_support::{
    call, create_chat, history, join, next_type, open_socket, register, serve_memory, Account,
    Server, Socket,
};
use reqwest::{Method, StatusCode};
use scheduled_support::{conversation, get_json, in_seconds, schedule, schedule_ok, send_now};
use serde_json::{json, Value};
use tokio_tungstenite::tungstenite::Message;
use uuid::Uuid;

async fn send_frame(socket: &mut Socket, frame: Value) {
    socket.send(Message::Text(frame.to_string())).await.unwrap();
}

/// The next `broadcast` whose content is `content`.
async fn broadcast_with(socket: &mut Socket, content: &str) -> Value {
    loop {
        let frame = next_type(socket, "broadcast").await;
        if frame["content"] == content {
            return frame;
        }
    }
}

async fn notification_kinds(base: &str, account: &Account) -> Vec<String> {
    get_json(base, account, "/api/notifications").await["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["kind"].as_str().unwrap().to_string())
        .collect()
}

/// Give `account` one Web Push subscription straight in the database, so push-job creation
/// (the `notifications` insert trigger) becomes observable without a push service.
async fn subscribe_push(server: &Server, account: &Account) {
    let now = chrono::Utc::now();
    sqlx::query(
        "INSERT INTO push_subscriptions (id, user_id, endpoint, p256dh, auth, created_at, \
         updated_at) VALUES ($1, $2, $3, 'k', 'a', $4, $4)",
    )
    .bind(Uuid::new_v4())
    .bind(account.id)
    .bind(format!("https://push.invalid/{}", Uuid::new_v4()))
    .bind(now)
    .execute(server.state.pool())
    .await
    .unwrap();
}

async fn push_jobs(server: &Server) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM push_delivery_jobs")
        .fetch_one(server.state.pool())
        .await
        .unwrap()
}

#[tokio::test]
async fn a_silent_reply_with_a_mention_notifies_nobody_and_queues_no_push() {
    let server = serve_memory().await;
    let base = &server.base;
    let alice = register(base, "tg404silentalice").await;
    let bob = register(base, "tg404silentbob").await;
    let chat = create_chat(base, &alice, "tg404 silent").await;
    join(base, &bob, chat).await;
    subscribe_push(&server, &bob).await;
    let mut bob_socket = open_socket(base, chat, &bob).await;
    let mut alice_socket = open_socket(base, chat, &alice).await;

    send_frame(
        &mut bob_socket,
        json!({ "type": "message", "content": "question?" }),
    )
    .await;
    let question = broadcast_with(&mut alice_socket, "question?").await;
    let question_id = question["message_id"].as_str().unwrap().to_string();

    send_frame(
        &mut alice_socket,
        json!({ "type": "message", "content": "quiet @tg404silentbob",
                "reply_to": question_id, "silent": true }),
    )
    .await;
    let quiet = broadcast_with(&mut bob_socket, "quiet @tg404silentbob").await;
    assert_eq!(quiet["silent"], true, "the wire carries the marker");
    assert!(quiet["reply_to"].is_object(), "still a normal reply");

    // The silent message is otherwise ordinary: in history with the marker, and unread.
    let stored = history(base, &bob, chat).await;
    let stored = stored
        .iter()
        .find(|message| message["id"] == quiet["message_id"])
        .expect("in history");
    assert_eq!(stored["silent"], true);
    assert!(
        conversation(base, &bob, chat).await["unread_count"]
            .as_i64()
            .unwrap()
            >= 1
    );
    assert!(
        notification_kinds(base, &bob).await.is_empty(),
        "silent: no reply/mention"
    );
    assert_eq!(push_jobs(&server).await, 0, "silent: no Web Push job");

    // Control: the same reply sent normally does notify and queue a push.
    send_frame(
        &mut alice_socket,
        json!({ "type": "message", "content": "loud @tg404silentbob",
                "reply_to": question_id }),
    )
    .await;
    let loud = broadcast_with(&mut bob_socket, "loud @tg404silentbob").await;
    assert!(loud.get("silent").is_none(), "omitted when false");
    let mut kinds = Vec::new();
    for _ in 0..50 {
        kinds = notification_kinds(base, &bob).await;
        if kinds.len() >= 2 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    kinds.sort();
    assert_eq!(kinds, ["mention", "reply"]);
    assert!(push_jobs(&server).await >= 2);
}

#[tokio::test]
async fn the_scheduler_delivers_on_time_through_the_normal_message_path() {
    let server = serve_memory().await;
    let base = &server.base;
    let alice = register(base, "tg404sched-alice").await;
    let bob = register(base, "tg404sched-bob").await;
    let chat = create_chat(base, &alice, "tg404 scheduler").await;
    join(base, &bob, chat).await;
    subscribe_push(&server, &bob).await;
    let mut bob_socket = open_socket(base, chat, &bob).await;

    send_frame(
        &mut bob_socket,
        json!({ "type": "message", "content": "ping" }),
    )
    .await;
    let bob_message = broadcast_with(&mut bob_socket, "ping").await["message_id"]
        .as_str()
        .unwrap()
        .to_string();
    let (status, body) = schedule(
        base,
        &alice,
        chat,
        json!({ "content": "scheduled silent reply", "scheduled_at": in_seconds(2),
                "silent": true, "reply_to": bob_message }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let id = body["id"].as_str().unwrap().to_string();

    let frame = broadcast_with(&mut bob_socket, "scheduled silent reply").await;
    assert_eq!(frame["message_id"], id, "delivered under the scheduled id");
    assert_eq!(frame["silent"], true);
    assert_eq!(frame["reply_to"]["message_id"], bob_message);
    assert!(notification_kinds(base, &bob).await.is_empty());
    assert_eq!(push_jobs(&server).await, 0);

    // Exactly once: later ticks find nothing.
    tokio::time::sleep(Duration::from_secs(2)).await;
    let delivered = history(base, &bob, chat)
        .await
        .into_iter()
        .filter(|message| message["id"] == id.as_str())
        .count();
    assert_eq!(delivered, 1);
}

#[tokio::test]
async fn a_scheduled_message_does_not_unarchive_the_chat_before_delivery() {
    let server = serve_memory().await;
    let base = &server.base;
    let alice = register(base, "tg404arch-alice").await;
    let bob = register(base, "tg404arch-bob").await;
    let chat = create_chat(base, &alice, "tg404 archive").await;
    join(base, &bob, chat).await;
    let (status, _) = call(
        Method::PATCH,
        format!("{base}/api/conversations/{chat}/preferences"),
        &bob.token,
        Some(json!({ "is_archived": true })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let archived = |row: Value| row["preferences"]["is_archived"].as_bool().unwrap();

    let id = schedule_ok(base, &alice, chat, "wake up later", 3600, false).await;
    assert!(
        archived(conversation(base, &bob, chat).await),
        "scheduling must not fire the unarchive trigger"
    );
    let (status, _) = send_now(base, &alice, chat, id).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        !archived(conversation(base, &bob, chat).await),
        "delivery is a real message insert and unarchives as usual"
    );
}
