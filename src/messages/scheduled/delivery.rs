//! Delivery of due scheduled messages.
//!
//! The dispatcher is a process-wide loop per `AppState` that polls `scheduled_messages` for due
//! rows every [`DELIVERY_POLL_INTERVAL`]. The table is the durable queue: nothing lives only in
//! memory, so a restart simply resumes (rows that fell due while the server was down go out on
//! the first tick). Each delivery holds a message-write permit from `work_queue`, the same
//! admission control every client message write passes, and skips ticks during maintenance
//! (backup / restore hold every permit anyway).
//!
//! The loop keeps only a `Weak` reference, so it ends when its `AppState` is dropped; the list
//! of started states lives here rather than in `AppState` to stay out of the state hotspot
//! (same pattern as `messages::polls::broadcast`).

use std::sync::{Arc, LazyLock, Mutex, Weak};
use std::time::Duration;

use chrono::Utc;
use uuid::Uuid;

use super::store::{Delivered, Delivery, ScheduledRow};
use crate::models::StoredMessage;
use crate::realtime::inbound::extract_mentions;
use crate::state::AppState;

/// How often due scheduled messages are looked for. Telegram delivers to the minute; one
/// second keeps «send at 12:00» visibly on time without a noticeable query load (the lookup is
/// one indexed range scan).
pub const DELIVERY_POLL_INTERVAL: Duration = Duration::from_secs(1);
const BATCH: i64 = 50;

static STARTED: LazyLock<Mutex<Vec<Weak<AppState>>>> = LazyLock::new(Default::default);

pub(crate) fn ensure_dispatcher(state: Arc<AppState>) {
    {
        let mut started = STARTED
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        started.retain(|weak| weak.strong_count() > 0);
        if started
            .iter()
            .any(|weak| std::ptr::eq(weak.as_ptr(), Arc::as_ptr(&state)))
        {
            return;
        }
        started.push(Arc::downgrade(&state));
    }
    let weak = Arc::downgrade(&state);
    drop(state);
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(DELIVERY_POLL_INTERVAL).await;
            let Some(state) = weak.upgrade() else { return };
            if state.maintenance_active() {
                continue;
            }
            if let Err(error) = deliver_due(&state).await {
                tracing::warn!("scheduled message delivery tick failed: {error}");
            }
        }
    });
}

async fn deliver_due(state: &AppState) -> Result<(), sqlx::Error> {
    for id in state.due_scheduled_ids(Utc::now(), BATCH).await? {
        let Ok(_permit) = state.work_queue().message().await else {
            tracing::warn!("scheduled delivery write queue timed out; retrying next tick");
            return Ok(());
        };
        let Some(row) = state.scheduled_by_id(id).await? else {
            continue;
        };
        deliver(state, row).await?;
    }
    Ok(())
}

/// Deliver one scheduled message now. Returns the delivered message as its author sees it, or
/// `None` when the row was already consumed or the author may no longer send in the chat (the
/// row is then discarded: a scheduled message never goes out under revoked rights).
pub(crate) async fn deliver(
    state: &AppState,
    row: ScheduledRow,
) -> Result<Option<StoredMessage>, sqlx::Error> {
    let Some(sender) = state.user_by_id(row.sender_id).await? else {
        return Ok(None);
    };
    let may_send = state
        .has_chat_permission(row.room_id, sender.id, "message.send")
        .await?
        || state
            .has_chat_permission(row.room_id, sender.id, "message.post")
            .await?;
    let sender_name = state.resolve_display_name(row.room_id, &sender).await;
    let reply_to = state
        .reply_preview(row.room_id, row.reply_to_id)
        .await?
        .map(|reply| reply.message_id);
    let entities = row.entities();
    if !may_send {
        discard(state, &row).await?;
        return Ok(None);
    }
    let outcome = state
        .deliver_scheduled_row(Delivery {
            row: &row,
            sender_name: &sender_name,
            reply_to,
            entities: &entities,
            created_at: Utc::now(),
        })
        .await?;
    match outcome {
        Delivered::Inserted => {}
        Delivered::Dropped => {
            tracing::info!(scheduled_id = %row.id, "scheduled message dropped: author left the chat");
            return Ok(None);
        }
        Delivered::Gone => return Ok(None),
    }
    state.invalidate_message_cache(row.room_id).await;
    record_mentions(state, &row, sender.id).await;
    state.message_by_id(row.id, Some(sender.id)).await
}

async fn discard(state: &AppState, row: &ScheduledRow) -> Result<(), sqlx::Error> {
    state
        .delete_scheduled(row.room_id, row.id, row.sender_id)
        .await?;
    tracing::info!(scheduled_id = %row.id, "scheduled message dropped: author may no longer send");
    Ok(())
}

async fn record_mentions(state: &AppState, row: &ScheduledRow, sender_id: Uuid) {
    let participants = state
        .chat_participants(row.room_id)
        .await
        .unwrap_or_default();
    let mentions = extract_mentions(&row.content, &participants, sender_id);
    if mentions.is_empty() {
        return;
    }
    if let Err(error) = state.record_message_mentions(row.id, &mentions).await {
        tracing::warn!("record scheduled message mentions failed: {error}");
    }
}
