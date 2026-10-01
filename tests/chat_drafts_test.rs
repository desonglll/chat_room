//! TG-008 HTTP contract: `PUT`/`GET /api/chats/:id/draft`, its idempotence and its validation.

use std::sync::Arc;

use chat_room::{build_app, state::AppState};
use tokio::{net::TcpListener, task::JoinHandle};
use uuid::Uuid;

mod support;
use support::session_token;

async fn start_server() -> (String, Arc<AppState>, JoinHandle<()>) {
    let state = Arc::new(AppState::new().await.unwrap());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let app = build_app(state.clone());
    let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (format!("http://{address}"), state, task)
}

async fn create_chat(base: &str, token: &str, title: &str) -> String {
    reqwest::Client::new()
        .post(format!("{base}/api/chats"))
        .bearer_auth(token)
        .json(&serde_json::json!({ "title": title, "join_policy": "open" }))
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string()
}

async fn put_draft(
    base: &str,
    prefix: &str,
    chat_id: &str,
    token: &str,
    body: serde_json::Value,
) -> (reqwest::StatusCode, serde_json::Value) {
    let response = reqwest::Client::new()
        .put(format!("{base}{prefix}/{chat_id}/draft"))
        .bearer_auth(token)
        .json(&body)
        .send()
        .await
        .unwrap();
    let status = response.status();
    let body = response.text().await.unwrap();
    (
        status,
        serde_json::from_str(&body).unwrap_or(serde_json::Value::Null),
    )
}

async fn get_draft(
    base: &str,
    prefix: &str,
    chat_id: &str,
    token: &str,
) -> (reqwest::StatusCode, serde_json::Value) {
    let response = reqwest::Client::new()
        .get(format!("{base}{prefix}/{chat_id}/draft"))
        .bearer_auth(token)
        .send()
        .await
        .unwrap();
    let status = response.status();
    let body = response.text().await.unwrap();
    (
        status,
        serde_json::from_str(&body).unwrap_or(serde_json::Value::Null),
    )
}

#[tokio::test]
async fn a_draft_survives_the_round_trip_and_the_identical_put_is_idempotent() {
    let (base, _state, _task) = start_server().await;
    let token = session_token(&base, "draft-owner").await;
    let chat_id = create_chat(&base, &token, "draft-chat").await;

    // Reconnect read path before anything is saved: 200 with JSON null, not an error.
    let (status, body) = get_draft(&base, "/api/chats", &chat_id, &token).await;
    assert_eq!(status, 200);
    assert_eq!(body, serde_json::Value::Null);

    let (status, saved) = put_draft(
        &base,
        "/api/chats",
        &chat_id,
        &token,
        serde_json::json!({ "text": "unsent thought " }),
    )
    .await;
    assert_eq!(status, 200);
    // Stored verbatim: trailing whitespace is legitimate mid-composition state.
    assert_eq!(saved["text"], "unsent thought ");
    assert_eq!(saved["reply_to_message_id"], serde_json::Value::Null);
    assert_eq!(saved["topic_id"], serde_json::Value::Null);
    let first_updated_at = saved["updated_at"].as_str().unwrap().to_string();

    // The identical PUT writes nothing: the stored timestamp survives.
    let (status, replay) = put_draft(
        &base,
        "/api/chats",
        &chat_id,
        &token,
        serde_json::json!({ "text": "unsent thought " }),
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(replay["updated_at"], first_updated_at.as_str());

    // The reconnect read path returns exactly what was stored.
    let (status, fetched) = get_draft(&base, "/api/chats", &chat_id, &token).await;
    assert_eq!(status, 200);
    assert_eq!(fetched, replay);

    // A different text is a real write: fresh timestamp.
    let (_, changed) = put_draft(
        &base,
        "/api/chats",
        &chat_id,
        &token,
        serde_json::json!({ "text": "unsent thought, finished" }),
    )
    .await;
    assert_ne!(changed["updated_at"], first_updated_at.as_str());

    // Clearing: empty text with no reply deletes the row; GET goes back to null.
    let (status, cleared) = put_draft(
        &base,
        "/api/chats",
        &chat_id,
        &token,
        serde_json::json!({ "text": "" }),
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(cleared["text"], "");
    let (status, body) = get_draft(&base, "/api/chats", &chat_id, &token).await;
    assert_eq!(status, 200);
    assert_eq!(body, serde_json::Value::Null);
    // Clearing again is idempotent, not an error.
    let (status, _) = put_draft(
        &base,
        "/api/chats",
        &chat_id,
        &token,
        serde_json::json!({ "text": "" }),
    )
    .await;
    assert_eq!(status, 200);
}

#[tokio::test]
async fn drafts_are_validated_and_authorized() {
    let (base, state, _task) = start_server().await;
    let owner = session_token(&base, "draft-auth-owner").await;
    let outsider = session_token(&base, "draft-auth-outsider").await;
    let chat_id = create_chat(&base, &owner, "draft-auth-chat").await;
    let other_chat = create_chat(&base, &owner, "draft-auth-other").await;

    // No session: 401. Non-member: 403. Unknown chat: 404.
    let bare = reqwest::Client::new()
        .put(format!("{base}/api/chats/{chat_id}/draft"))
        .json(&serde_json::json!({ "text": "x" }))
        .send()
        .await
        .unwrap();
    assert_eq!(bare.status(), 401);
    let (status, _) = put_draft(
        &base,
        "/api/chats",
        &chat_id,
        &outsider,
        serde_json::json!({ "text": "x" }),
    )
    .await;
    assert_eq!(status, 403);
    let (status, _) = get_draft(&base, "/api/chats", &chat_id, &outsider).await;
    assert_eq!(status, 403);
    let (status, _) = put_draft(
        &base,
        "/api/chats",
        &Uuid::new_v4().to_string(),
        &owner,
        serde_json::json!({ "text": "x" }),
    )
    .await;
    assert_eq!(status, 404);

    // Over the 4096-char message cap: refused, not truncated.
    let (status, _) = put_draft(
        &base,
        "/api/chats",
        &chat_id,
        &owner,
        serde_json::json!({ "text": "x".repeat(4097) }),
    )
    .await;
    assert_eq!(status, 400);
    let (_, body) = get_draft(&base, "/api/chats", &chat_id, &owner).await;
    assert_eq!(
        body,
        serde_json::Value::Null,
        "a refused draft is not stored"
    );

    // A reply target must be a live message of THIS chat.
    let (status, _) = put_draft(
        &base,
        "/api/chats",
        &chat_id,
        &owner,
        serde_json::json!({ "text": "re:", "reply_to_message_id": Uuid::new_v4() }),
    )
    .await;
    assert_eq!(status, 400, "an unknown reply target is refused");

    let owner_id = state
        .session_user(Uuid::parse_str(&owner).unwrap())
        .await
        .unwrap()
        .unwrap()
        .id;
    let insert_message = |chat: String, recalled: bool| {
        let state = state.clone();
        async move {
            let id = Uuid::new_v4();
            let now = chrono::Utc::now();
            sqlx::query(
                "INSERT INTO messages (id, room_id, sender_id, sender, content, recalled_at, \
                 created_at) VALUES (?, ?, ?, 'draft-auth-owner', 'target', ?, ?)",
            )
            .bind(id)
            .bind(Uuid::parse_str(&chat).unwrap())
            .bind(owner_id)
            .bind(recalled.then_some(now))
            .bind(now)
            .execute(state.pool())
            .await
            .unwrap();
            id
        }
    };
    let cross_chat_reply = insert_message(other_chat.clone(), false).await;
    let (status, _) = put_draft(
        &base,
        "/api/chats",
        &chat_id,
        &owner,
        serde_json::json!({ "text": "re:", "reply_to_message_id": cross_chat_reply }),
    )
    .await;
    assert_eq!(
        status, 400,
        "a reply pointer into another chat crosses the authorization boundary"
    );

    let recalled_reply = insert_message(chat_id.clone(), true).await;
    let (status, _) = put_draft(
        &base,
        "/api/chats",
        &chat_id,
        &owner,
        serde_json::json!({ "text": "re:", "reply_to_message_id": recalled_reply }),
    )
    .await;
    assert_eq!(status, 400, "a recalled message is not a reply target");

    // A live message of this chat IS a valid target — and an empty text WITH a reply is a
    // stored draft (reply picked, no text yet), not a clear.
    let live_reply = insert_message(chat_id.clone(), false).await;
    let (status, saved) = put_draft(
        &base,
        "/api/chats",
        &chat_id,
        &owner,
        serde_json::json!({ "text": "", "reply_to_message_id": live_reply }),
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(saved["reply_to_message_id"], live_reply.to_string());
    let (_, fetched) = get_draft(&base, "/api/chats", &chat_id, &owner).await;
    assert_eq!(
        fetched["reply_to_message_id"],
        live_reply.to_string(),
        "the reply-only draft is stored"
    );
}
