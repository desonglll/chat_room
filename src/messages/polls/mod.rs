//! Polls and quizzes (TG-406).
//!
//! A poll is a projection keyed by the message that carries it: the `messages` row stays the
//! source of truth for chat, sender, order, recall and history, so a poll message flows through
//! the normal history and realtime paths and only gains an optional `poll` field
//! (`StoredMessage::poll`, the `broadcast` frame's `poll`). This task does not use TG-302's
//! `messages.media_kind`; "is a poll" means "has a `polls` row".
//!
//! The functions below are the module interface. HTTP handlers translate and call them; they
//! own authorisation, the transaction, and the aggregated `poll_updated` broadcast.
//! Contract: `docs/devlog/TG-406.md`, Frozen interface.

mod access;
mod broadcast;
pub(crate) mod handlers;
pub mod model;
mod read;
mod write;

use std::sync::Arc;

use axum::{
    routing::{get, post},
    Router,
};
use uuid::Uuid;

pub use broadcast::POLL_BROADCAST_WINDOW;
pub(crate) use read::Audience;
pub(crate) use write::{FORWARD_COPY_POLL, FORWARD_COPY_POLL_OPTIONS};

use crate::models::{PollState, StoredMessage, User};
use crate::state::{AppState, SharedState};
use broadcast::schedule_poll_broadcast;
use model::{CreatePollRequest, PollError, PollVoterPage};

/// The poll routes. `POST /api/chats/:id/polls` is chat-scoped but only on the canonical
/// prefix: the deprecated `/api/rooms` alias serves frozen clients that know no polls.
pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/chats/:id/polls", post(handlers::create))
        .route("/api/polls/:message_id", get(handlers::get))
        .route(
            "/api/polls/:message_id/votes",
            post(handlers::vote).delete(handlers::retract),
        )
        .route("/api/polls/:message_id/close", post(handlers::close))
        .route("/api/polls/:message_id/voters", get(handlers::voters))
}

/// Create a poll message in `room_id` as `sender`. The message and its poll are written in one
/// transaction; the message then reaches every connection through the normal message path.
pub async fn create_poll(
    state: &SharedState,
    room_id: Uuid,
    sender: &User,
    request: &CreatePollRequest,
) -> Result<StoredMessage, PollError> {
    let poll = request.validate()?;
    state.chat(room_id).await.ok_or(PollError::NotFound)?;
    if !state.is_chat_participant(room_id, sender.id).await? {
        return Err(PollError::NotFound);
    }
    // TG-202: `message.send_poll`, whose prerequisite is `message.send` — decided as
    // `message.post` in a channel (`ChatType::effective_permission`).
    if !state
        .has_chat_permission(room_id, sender.id, "message.send_poll")
        .await?
    {
        return Err(PollError::Forbidden);
    }
    let display_name = state.resolve_display_name(room_id, sender).await;
    let message_id = state
        .insert_poll_message(
            room_id,
            sender,
            &display_name,
            &poll,
            request.reply_to,
            request.client_message_id,
        )
        .await?
        .ok_or(PollError::Forbidden)?;
    let message = state
        .message_by_id(message_id, Some(sender.id))
        .await?
        .ok_or(PollError::NotFound)?;
    Ok(message)
}

/// The poll as `viewer` sees it.
pub async fn poll_for_viewer(
    state: &SharedState,
    message_id: Uuid,
    viewer: Uuid,
) -> Result<PollState, PollError> {
    state.poll_access(message_id, viewer).await?;
    viewer_snapshot(state, message_id, viewer).await
}

/// Cast or replace `user_id`'s ballot, then schedule the aggregated broadcast.
pub async fn cast_vote(
    state: &SharedState,
    message_id: Uuid,
    user_id: Uuid,
    options: &[u32],
) -> Result<PollState, PollError> {
    let access = state.poll_access(message_id, user_id).await?;
    let ballot = access.ballot(options)?;
    state.record_poll_vote(&access, user_id, &ballot).await?;
    schedule_poll_broadcast(state, access.room_id, message_id);
    viewer_snapshot(state, message_id, user_id).await
}

/// Retract `user_id`'s ballot (not allowed in a quiz).
pub async fn retract_vote(
    state: &SharedState,
    message_id: Uuid,
    user_id: Uuid,
) -> Result<PollState, PollError> {
    let access = state.poll_access(message_id, user_id).await?;
    if state.retract_poll_vote(&access, user_id).await? {
        schedule_poll_broadcast(state, access.room_id, message_id);
    }
    viewer_snapshot(state, message_id, user_id).await
}

/// Stop the poll. The poll's author or a chat administrator only.
pub async fn close_poll(
    state: &SharedState,
    message_id: Uuid,
    user_id: Uuid,
) -> Result<PollState, PollError> {
    let access = state.poll_access(message_id, user_id).await?;
    if !state.may_close_poll(&access, user_id).await? {
        return Err(PollError::Forbidden);
    }
    if state.close_poll_row(message_id).await? {
        schedule_poll_broadcast(state, access.room_id, message_id);
    }
    viewer_snapshot(state, message_id, user_id).await
}

/// One page of an option's voters. Anonymous polls refuse — for everyone, the author included.
pub async fn list_voters(
    state: &SharedState,
    message_id: Uuid,
    viewer: Uuid,
    option: u32,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<PollVoterPage, PollError> {
    let access = state.poll_access(message_id, viewer).await?;
    if !access.public_voters {
        return Err(PollError::Forbidden);
    }
    if option as usize >= access.option_count {
        return Err(PollError::Invalid);
    }
    let limit = limit.unwrap_or(50).clamp(1, 100);
    let offset = offset.unwrap_or(0).max(0);
    Ok(state
        .poll_voter_page(message_id, option, limit, offset)
        .await?)
}

async fn viewer_snapshot(
    state: &AppState,
    message_id: Uuid,
    viewer: Uuid,
) -> Result<PollState, PollError> {
    state
        .poll_states(&[message_id], Audience::Viewer(viewer))
        .await?
        .remove(&message_id)
        .ok_or(PollError::NotFound)
}
