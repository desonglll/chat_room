//! TG-207 slow mode: in a supergroup with `slow_mode_seconds > 0`, a member may send one
//! message per interval. The server is the only authority — the client's countdown is a
//! convenience — so the rule is enforced in the post gate every send path already calls
//! (`AppState::resolve_post_topic`), by comparing with the member's newest message in the
//! chat. Owners and administrators are exempt (Telegram).
//!
//! Recalled messages still count, so deleting and resending cannot skip the wait. Scheduled
//! delivery is exempt (the message was accepted when it was scheduled), and so is scheduling
//! itself, which writes no message. Two sends racing inside the same instant can both pass;
//! the rule is a rate limit, not an invariant, and Telegram tolerates the same.

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use super::capabilities::{CapabilityError, ChatCapabilityChange};
use crate::models::Chat;
use crate::state::{with_pool, AppState, SharedState};

/// Telegram's choices: off, 10 s, 30 s, 1 min, 5 min, 15 min, 1 h.
pub const SLOW_MODE_CHOICES: [i64; 7] = [0, 10, 30, 60, 300, 900, 3600];

#[derive(Debug, Deserialize, ToSchema)]
pub struct SlowModeRequest {
    /// One of 0, 10, 30, 60, 300, 900, 3600. 0 turns slow mode off.
    pub seconds: i64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SlowModeState {
    pub seconds: i64,
    /// Seconds the caller still has to wait before the next message; 0 when they may send
    /// (always 0 for owners and administrators).
    pub wait_seconds: i64,
    pub exempt: bool,
}

impl AppState {
    /// Owners and administrators are not subject to slow mode.
    pub async fn slow_mode_exempt(
        &self,
        room_id: Uuid,
        user_id: Uuid,
    ) -> Result<bool, sqlx::Error> {
        Ok(matches!(
            self.membership_identity(room_id, user_id).await?,
            Some((status, role)) if status == "active" && matches!(role.as_str(), "owner" | "admin")
        ))
    }

    /// Seconds `user_id` must still wait before posting into `room_id`; 0 = may post now.
    pub async fn slow_mode_wait(&self, room_id: Uuid, user_id: Uuid) -> Result<i64, sqlx::Error> {
        let Some(chat) = self.chat(room_id).await else {
            return Ok(0);
        };
        if chat.slow_mode_seconds <= 0 || self.slow_mode_exempt(room_id, user_id).await? {
            return Ok(0);
        }
        let last: Option<DateTime<Utc>> = with_pool!(self, |pool| {
            sqlx::query_scalar(
                "SELECT MAX(created_at) FROM messages WHERE room_id = $1 AND sender_id = $2",
            )
            .bind(room_id)
            .bind(user_id)
            .fetch_one(pool)
            .await
        })?;
        let Some(last) = last else {
            return Ok(0);
        };
        let elapsed = (Utc::now() - last).num_seconds().max(0);
        Ok((chat.slow_mode_seconds - elapsed).max(0))
    }
}

#[utoipa::path(get, path = "/api/chats/{id}/slow-mode", params(("id" = Uuid, description = "Chat id")),
    responses((status = 200, description = "The chat's interval and the caller's remaining wait", body = SlowModeState),
        (status = 404, description = "Not a member")))]
pub async fn get_slow_mode(
    State(state): State<SharedState>,
    Path(room_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<SlowModeState>, StatusCode> {
    let user = super::membership_handlers::session_user(&state, &headers).await?;
    if !state
        .can_read_chat(room_id, user.id)
        .await
        .map_err(internal)?
    {
        return Err(StatusCode::NOT_FOUND);
    }
    let chat = state.chat(room_id).await.ok_or(StatusCode::NOT_FOUND)?;
    Ok(Json(SlowModeState {
        seconds: chat.slow_mode_seconds,
        wait_seconds: state
            .slow_mode_wait(room_id, user.id)
            .await
            .map_err(internal)?,
        exempt: state
            .slow_mode_exempt(room_id, user.id)
            .await
            .map_err(internal)?,
    }))
}

#[utoipa::path(put, path = "/api/chats/{id}/slow-mode", params(("id" = Uuid, description = "Chat id")),
    request_body = SlowModeRequest,
    responses((status = 200, description = "Slow mode set (a group becomes a supergroup)", body = Chat),
        (status = 400, description = "Not one of the allowed intervals"),
        (status = 403, description = "Missing members.ban"),
        (status = 409, description = "This chat type cannot have slow mode")))]
pub async fn put_slow_mode(
    State(state): State<SharedState>,
    Path(room_id): Path<Uuid>,
    headers: HeaderMap,
    Json(request): Json<SlowModeRequest>,
) -> Result<Json<Chat>, StatusCode> {
    if !SLOW_MODE_CHOICES.contains(&request.seconds) {
        return Err(StatusCode::BAD_REQUEST);
    }
    let user = super::membership_handlers::session_user(&state, &headers).await?;
    // Telegram puts slow mode under "Permissions", for admins who may restrict members.
    if !state
        .has_chat_permission(room_id, user.id, "members.ban")
        .await
        .map_err(internal)?
    {
        return Err(StatusCode::FORBIDDEN);
    }
    let change = ChatCapabilityChange {
        slow_mode_seconds: Some(request.seconds),
        ..Default::default()
    };
    match state.apply_chat_capabilities(room_id, change).await {
        Ok(outcome) => Ok(Json(outcome.chat)),
        Err(CapabilityError::NotFound) => Err(StatusCode::NOT_FOUND),
        Err(CapabilityError::NotSupported(_)) => Err(StatusCode::CONFLICT),
        Err(CapabilityError::Database(error)) => Err(internal(error)),
    }
}

fn internal(error: sqlx::Error) -> StatusCode {
    tracing::error!("slow mode: {error}");
    StatusCode::INTERNAL_SERVER_ERROR
}
