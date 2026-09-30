//! Aggregated `poll_updated` broadcasts.
//!
//! A vote never broadcasts on its own. It marks its poll dirty; the first mark opens a
//! [`POLL_BROADCAST_WINDOW`] and every vote that lands inside the window rides on the single
//! flush at its end, which reads the aggregate *after* clearing the mark. So:
//!
//! - every committed vote is reflected by a broadcast that starts after it (a vote that
//!   commits after the flush cleared the mark opens a new window);
//! - one poll produces at most one broadcast per window, however many voters there are —
//!   1000 concurrent voters over `t` seconds cost at most `t / window + 1` frames.
//!
//! The pending set is process-wide and keyed by message id. Message ids are globally unique,
//! so two `AppState`s in one process (tests) can never collide, and keeping it here instead of
//! in `AppState` keeps this task out of the shared state hotspot.

use std::collections::HashSet;
use std::sync::{LazyLock, Mutex};
use std::time::Duration;

use uuid::Uuid;

use super::read::Audience;
use crate::models::ChatMessage;
use crate::state::SharedState;

/// Coalescing window per poll.
pub const POLL_BROADCAST_WINDOW: Duration = Duration::from_millis(250);

static PENDING: LazyLock<Mutex<HashSet<Uuid>>> = LazyLock::new(Default::default);

fn pending() -> std::sync::MutexGuard<'static, HashSet<Uuid>> {
    // A panic while holding this lock cannot leave the set inconsistent (insert/remove only).
    PENDING
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Mark `message_id`'s poll as changed; a chat-wide snapshot follows within one window.
pub(crate) fn schedule_poll_broadcast(state: &SharedState, room_id: Uuid, message_id: Uuid) {
    if !pending().insert(message_id) {
        return;
    }
    let state = state.clone();
    tokio::spawn(async move {
        tokio::time::sleep(POLL_BROADCAST_WINDOW).await;
        pending().remove(&message_id);
        match state.poll_states(&[message_id], Audience::Chat).await {
            Ok(mut states) => {
                if let Some(poll) = states.remove(&message_id) {
                    state
                        .broadcast(room_id, ChatMessage::PollUpdated { message_id, poll })
                        .await;
                }
            }
            Err(error) => tracing::warn!(%message_id, "load poll for broadcast failed: {error}"),
        }
    });
}
