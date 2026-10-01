//! Scheduled and silent messages (TG-404).
//!
//! A scheduled message is **not** a message until it is delivered: it lives in
//! `scheduled_messages`, visible only to its author, and is inserted into `messages` (under the
//! same id) at delivery. So history, search, unread counts, chat-list previews, notifications
//! and every `messages` trigger stay untouched — nothing can see the message early because it
//! is not there. Delivery is exactly once (`store::deliver_scheduled_row`) and durable across
//! restarts (`delivery`).
//!
//! Silent is a flag on the delivered message (`messages.silent`, the `silent` wire field): the
//! message is normal in every respect except that the notification triggers skip it, and with
//! them Web Push, which is fed only from `notifications`.
//!
//! The functions below are the module interface; HTTP handlers only translate.
//! Contract: `docs/devlog/TG-404.md`, Frozen interface.

mod delivery;
pub(crate) mod handlers;
pub mod model;
mod store;

use std::sync::Arc;

use axum::{
    routing::{get, patch, post},
    Router,
};
use chrono::Utc;
use uuid::Uuid;

pub(crate) use delivery::ensure_dispatcher;
pub use delivery::DELIVERY_POLL_INTERVAL;

use crate::models::{StoredMessage, User};
use crate::realtime::auth::normalize_message;
use crate::state::{AppState, SharedState};
use crate::stickers::custom_emoji::{entities::leading_trim_utf16, MessageEntity};
use model::{
    validate_date, CreateScheduledMessageRequest, ScheduledError, ScheduledMessage,
    UpdateScheduledMessageRequest, MAX_SCHEDULED_PER_CHAT,
};
use store::{ScheduledPatch, ScheduledRow};

/// The scheduled-message routes.
pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/api/chats/:id/scheduled-messages",
            get(handlers::list).post(handlers::create),
        )
        .route(
            "/api/chats/:id/scheduled-messages/:scheduled_id",
            patch(handlers::update).delete(handlers::delete),
        )
        .route(
            "/api/chats/:id/scheduled-messages/:scheduled_id/send-now",
            post(handlers::send_now),
        )
}

/// Membership plus the right to send (`message.send`, or `message.post` in a channel).
async fn authorize_sender(
    state: &AppState,
    room_id: Uuid,
    user_id: Uuid,
) -> Result<(), ScheduledError> {
    state.chat(room_id).await.ok_or(ScheduledError::NotFound)?;
    if !state.is_chat_participant(room_id, user_id).await? {
        return Err(ScheduledError::NotFound);
    }
    let may_send = state
        .has_chat_permission(room_id, user_id, "message.send")
        .await?
        || state
            .has_chat_permission(room_id, user_id, "message.post")
            .await?;
    if may_send {
        Ok(())
    } else {
        Err(ScheduledError::Forbidden)
    }
}

async fn accept_text(
    state: &AppState,
    content: String,
    entities: Vec<MessageEntity>,
) -> Result<(String, String), ScheduledError> {
    let leading_trim = leading_trim_utf16(&content);
    let content = normalize_message(content).ok_or(ScheduledError::Invalid)?;
    let entities = state
        .accept_message_entities(&content, leading_trim, entities)
        .await;
    let entities = serde_json::to_string(&entities).map_err(|_| ScheduledError::Invalid)?;
    Ok((content, entities))
}

/// Schedule a text message in `room_id` for `sender`.
pub async fn schedule(
    state: &SharedState,
    room_id: Uuid,
    sender: &User,
    request: CreateScheduledMessageRequest,
) -> Result<ScheduledMessage, ScheduledError> {
    authorize_sender(state, room_id, sender.id).await?;
    let now = Utc::now();
    if !validate_date(request.scheduled_at, now) {
        return Err(ScheduledError::Invalid);
    }
    if state.count_scheduled(room_id, sender.id).await? >= MAX_SCHEDULED_PER_CHAT {
        return Err(ScheduledError::Limit);
    }
    let topic_id = state
        .resolve_scheduled_post_topic(room_id, sender.id, request.topic_id)
        .await
        .map_err(ScheduledError::from_topic)?;
    let (content, entities) = accept_text(state, request.content, request.entities).await?;
    let reply_to = state
        .reply_preview(room_id, request.reply_to)
        .await?
        .map(|reply| reply.message_id);
    let row = ScheduledRow {
        id: Uuid::new_v4(),
        room_id,
        sender_id: sender.id,
        content,
        entities,
        reply_to_id: reply_to,
        silent: request.silent,
        scheduled_at: request.scheduled_at,
        created_at: now,
        updated_at: now,
        topic_id,
    };
    state.insert_scheduled(&row).await?;
    Ok(row.into_view())
}

/// The caller's own scheduled messages in `room_id`, soonest first. Nobody else's.
pub async fn list(
    state: &SharedState,
    room_id: Uuid,
    viewer: Uuid,
) -> Result<Vec<ScheduledMessage>, ScheduledError> {
    state.chat(room_id).await.ok_or(ScheduledError::NotFound)?;
    if !state.is_chat_participant(room_id, viewer).await? {
        return Err(ScheduledError::NotFound);
    }
    Ok(state
        .list_scheduled(room_id, viewer)
        .await?
        .into_iter()
        .map(ScheduledRow::into_view)
        .collect())
}

/// Edit the text, reschedule, or toggle silent. Author only.
pub async fn update(
    state: &SharedState,
    room_id: Uuid,
    id: Uuid,
    author: &User,
    request: UpdateScheduledMessageRequest,
) -> Result<ScheduledMessage, ScheduledError> {
    authorize_sender(state, room_id, author.id).await?;
    let current = state
        .own_scheduled(room_id, id, author.id)
        .await?
        .ok_or(ScheduledError::NotFound)?;
    let (content, entities) = match request.content {
        Some(content) => accept_text(state, content, request.entities.unwrap_or_default()).await?,
        None => (current.content.clone(), current.entities.clone()),
    };
    let scheduled_at = match request.scheduled_at {
        Some(at) if !validate_date(at, Utc::now()) => return Err(ScheduledError::Invalid),
        Some(at) => at,
        None => current.scheduled_at,
    };
    let patch = ScheduledPatch {
        content,
        entities,
        scheduled_at,
        silent: request.silent.unwrap_or(current.silent),
    };
    if !state.update_scheduled(id, author.id, &patch).await? {
        return Err(ScheduledError::NotFound);
    }
    state
        .own_scheduled(room_id, id, author.id)
        .await?
        .map(ScheduledRow::into_view)
        .ok_or(ScheduledError::NotFound)
}

/// Cancel a scheduled message. Author only; membership is not required (leaving a chat must
/// not strand an undeletable row — delivery would discard it anyway).
pub async fn cancel(
    state: &SharedState,
    room_id: Uuid,
    id: Uuid,
    author: Uuid,
) -> Result<(), ScheduledError> {
    if state.delete_scheduled(room_id, id, author).await? {
        Ok(())
    } else {
        Err(ScheduledError::NotFound)
    }
}

/// «Send now»: deliver immediately, exactly as the scheduler would.
pub async fn send_now(
    state: &SharedState,
    room_id: Uuid,
    id: Uuid,
    author: &User,
) -> Result<StoredMessage, ScheduledError> {
    authorize_sender(state, room_id, author.id).await?;
    let row = state
        .own_scheduled(room_id, id, author.id)
        .await?
        .ok_or(ScheduledError::NotFound)?;
    let _permit = state
        .work_queue()
        .message()
        .await
        .map_err(|_| ScheduledError::Busy)?;
    delivery::deliver(state, row)
        .await?
        .ok_or(ScheduledError::NotFound)
}
