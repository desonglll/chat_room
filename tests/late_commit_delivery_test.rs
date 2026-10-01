//! TG-604: a message that commits after a newer one (its `created_at` behind the live cursor,
//! as happens under write contention) still reaches open sockets — once, and without
//! re-sending what the socket already had.

mod poll_support;

use std::time::Duration;

use chrono::Utc;
use poll_support::{create_chat, drain_type, next_type, open_socket, register, serve_memory};
use uuid::Uuid;

#[tokio::test]
async fn a_late_commit_behind_the_cursor_is_still_delivered_once() {
    let server = serve_memory().await;
    let base = &server.base;
    let alice = register(base, "late-commit-alice").await;
    let chat = create_chat(base, &alice, "late commit").await;
    let mut socket = open_socket(base, chat, &alice).await;

    let insert = |content: &'static str, created_at: chrono::DateTime<Utc>| {
        let state = server.state.clone();
        let sender = alice.id;
        async move {
            sqlx::query(
                "INSERT INTO messages (id, room_id, sender_id, sender, content, created_at) \
                 VALUES ($1, $2, $3, 'late-commit-alice', $4, $5)",
            )
            .bind(Uuid::new_v4())
            .bind(chat)
            .bind(sender)
            .bind(content)
            .bind(created_at)
            .execute(state.pool())
            .await
            .unwrap();
        }
    };

    insert("newer", Utc::now()).await;
    let first = next_type(&mut socket, "broadcast").await;
    assert_eq!(first["content"], "newer");

    // Committed now, stamped two seconds earlier: behind what the socket's cursor has passed.
    insert("late", Utc::now() - chrono::Duration::seconds(2)).await;
    let late = next_type(&mut socket, "broadcast").await;
    assert_eq!(late["content"], "late", "{late}");

    let again = drain_type(&mut socket, "broadcast", Duration::from_millis(1200)).await;
    assert!(again.is_empty(), "nothing is delivered twice: {again:?}");
}
