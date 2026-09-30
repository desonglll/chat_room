//! TG-110: a poll message cannot be edited (Telegram). The WebSocket `edit` frame for a poll
//! is ignored — no `message_edited` broadcast, and the stored content keeps the question —
//! while an ordinary text message in the same chat still edits normally. Both adapters: the
//! guard is a `NOT EXISTS` sub-select inside the edit's `UPDATE`.

mod migration_support;
mod poll_support;

use std::sync::Arc;

use chat_room::config::AppConfig;
use chat_room::state::AppState;
use futures_util::SinkExt;
use migration_support::{create_postgres_scratch, drop_postgres_scratch, postgres_admin_pool};
use poll_support::*;
use serde_json::json;
use tokio_tungstenite::tungstenite::Message;

#[tokio::test]
async fn editing_a_poll_message_is_rejected_on_sqlite() {
    edit_scenario(&serve_memory().await).await;
}

#[tokio::test]
async fn editing_a_poll_message_is_rejected_on_postgres() {
    let Some((admin_url, admin_pool)) =
        postgres_admin_pool("editing_a_poll_message_is_rejected_on_postgres").await
    else {
        return;
    };
    let scratch = create_postgres_scratch(&admin_url, &admin_pool, "poll_edit").await;
    let state = AppState::open_postgres(&scratch.url, &AppConfig::default())
        .await
        .unwrap();
    let server = serve(Arc::new(state)).await;
    edit_scenario(&server).await;
    server.state.postgres_pool().unwrap().close().await;
    drop(server);
    drop_postgres_scratch(&admin_pool, &scratch).await;
}

async fn edit_scenario(server: &Server) {
    let base = &server.base;
    let alice = register(base, "edit-alice").await;
    let chat = create_chat(base, &alice, "edits").await;
    let poll_id = poll(
        base,
        &alice,
        chat,
        json!({ "question": "Lunch?", "options": ["noodles", "rice"] }),
    )
    .await;
    let mut socket = open_socket(base, chat, &alice).await;

    socket
        .send(Message::Text(
            json!({ "type": "message", "content": "typo" }).to_string(),
        ))
        .await
        .unwrap();
    let text_id = next_type(&mut socket, "broadcast").await["message_id"]
        .as_str()
        .unwrap()
        .to_owned();

    // The poll edit first, then the text edit: the only `message_edited` frame that arrives
    // must be the text one — the poll edit produced nothing.
    for (id, content) in [
        (poll_id.to_string(), "hijacked"),
        (text_id.clone(), "fixed"),
    ] {
        socket
            .send(Message::Text(
                json!({ "type": "edit", "message_id": id, "content": content }).to_string(),
            ))
            .await
            .unwrap();
    }
    let edited = next_type(&mut socket, "message_edited").await;
    assert_eq!(edited["message_id"], text_id.as_str());
    assert_eq!(edited["content"], "fixed");

    let direct = server
        .state
        .edit_message(chat, alice.id, poll_id, "hijacked again")
        .await
        .unwrap();
    assert!(direct.is_none(), "a poll message must not be editable");

    let messages = history(base, &alice, chat).await;
    let stored = messages
        .iter()
        .find(|message| message["id"] == poll_id.to_string().as_str())
        .expect("poll message in history");
    assert!(stored["edited_at"].is_null());
    assert_ne!(stored["content"], "hijacked");
    assert_ne!(stored["content"], "hijacked again");
    assert!(stored["poll"].is_object());
}
