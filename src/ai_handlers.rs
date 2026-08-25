//! AI suggestions and authorized room-context preparation.

use std::collections::HashSet;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use uuid::Uuid;

use crate::ai::{bounded_conversation_context_to_toon, AiContextMessage, AiSuggestions};
use crate::handlers::authorize_room;
use crate::models::Room;
use crate::state::SharedState;
use crate::user_handlers::bearer_token;

/// Summarize the recent conversation and suggest a few replies the caller might
/// send next.
#[utoipa::path(
    post,
    path = "/api/rooms/{id}/ai/suggest",
    params(("id" = Uuid, Path, description = "Room ID")),
    responses(
        (status = 200, description = "Summary and suggested replies", body = AiSuggestions),
        (status = 401, description = "Missing or expired session"),
        (status = 403, description = "Not permitted to post in this room"),
        (status = 404, description = "Room not found"),
        (status = 429, description = "Too many requests, try again shortly"),
        (status = 503, description = "AI assistant is disabled or unavailable")
    )
)]
pub async fn suggest(
    State(state): State<SharedState>,
    Path(room_id): Path<Uuid>,
    headers: axum::http::HeaderMap,
) -> Result<Json<AiSuggestions>, StatusCode> {
    let token = bearer_token(&headers)?;
    let user = state
        .session_user(token)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::UNAUTHORIZED)?;

    let room = state.room(room_id).await.ok_or(StatusCode::NOT_FOUND)?;

    let can_send = state
        .has_room_permission(room_id, user.id, "message.send")
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if !can_send {
        return Err(StatusCode::FORBIDDEN);
    }
    require_room_password(&room, &headers)?;
    let Some(assistant) = state.ai_assistant() else {
        return Err(StatusCode::SERVICE_UNAVAILABLE);
    };

    if !state
        .check_action_cooldown(room_id, user.id, user.id, state.ai_suggest_cooldown())
        .await
    {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }

    let limit = state.ai_max_context_messages();
    let history = state
        .message_history(room_id, limit as i64, None, Some(user.id))
        .await
        .map_err(|error| {
            tracing::error!("load AI context history failed: {}", error);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let context: Vec<AiContextMessage> = history
        .into_iter()
        .filter(|message| message.recalled_at.is_none())
        .map(|message| AiContextMessage {
            sent_at: message.created_at.to_rfc3339(),
            sender: message.sender,
            content: message.content,
            attachment: message
                .attachment
                .map(|attachment| attachment.file_name)
                .unwrap_or_default(),
        })
        .collect();

    match assistant.suggest(&room.name, &context).await {
        Ok(suggestions) => Ok(Json(suggestions)),
        Err(error) => {
            tracing::error!("AI suggest request failed: {}", error);
            Err(StatusCode::SERVICE_UNAVAILABLE)
        }
    }
}

const MAX_CONTEXT_MESSAGE_CHARS: usize = 1_500;
const MAX_CONTEXT_TOON_BYTES: usize = 256 * 1024;

pub(crate) struct PreparedRoomContext {
    pub toon_context: String,
    pub context_message_count: usize,
    pub message_ids: HashSet<Uuid>,
}

pub(crate) async fn room_context_for_user(
    state: &SharedState,
    user_id: Uuid,
    room_id: Uuid,
    headers: &axum::http::HeaderMap,
) -> Result<PreparedRoomContext, StatusCode> {
    let room = state.room(room_id).await.ok_or(StatusCode::NOT_FOUND)?;
    state
        .conversation_summary(user_id, room_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::FORBIDDEN)?;
    require_room_password(&room, headers)?;
    room_context_for_authorized_user(state, user_id, room_id).await
}

pub(crate) async fn room_context_for_authorized_user(
    state: &SharedState,
    user_id: Uuid,
    room_id: Uuid,
) -> Result<PreparedRoomContext, StatusCode> {
    let room = state.room(room_id).await.ok_or(StatusCode::NOT_FOUND)?;
    let conversation = state
        .conversation_summary(user_id, room_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::FORBIDDEN)?;
    let history = state
        .message_history(
            room.id,
            state.ai_analysis_context_messages() as i64,
            None,
            Some(user_id),
        )
        .await
        .map_err(|error| {
            tracing::error!(%room_id, "load AI analysis context failed: {error}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    let message_ids = history
        .iter()
        .filter(|message| message.recalled_at.is_none())
        .map(|message| message.id)
        .collect();
    let mut context: Vec<AiContextMessage> = history
        .into_iter()
        .filter(|message| message.recalled_at.is_none())
        .map(|message| AiContextMessage {
            sent_at: message.created_at.to_rfc3339(),
            sender: message.sender,
            content: truncate_chars(&message.content, MAX_CONTEXT_MESSAGE_CHARS),
            attachment: message
                .attachment
                .map(|attachment| attachment.file_name)
                .unwrap_or_default(),
        })
        .collect();
    let toon_context = bounded_conversation_context_to_toon(
        &conversation.title,
        &mut context,
        MAX_CONTEXT_TOON_BYTES,
    )
    .map_err(|error| {
        tracing::error!(%room_id, "encode AI analysis context failed: {error}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    Ok(PreparedRoomContext {
        toon_context,
        context_message_count: context.len(),
        message_ids,
    })
}

fn require_room_password(room: &Room, headers: &axum::http::HeaderMap) -> Result<(), StatusCode> {
    if !room.has_password {
        return Ok(());
    }
    let supplied = headers
        .get("x-room-password")
        .and_then(|value| value.to_str().ok());
    authorize_room(room, supplied)
        .then_some(())
        .ok_or(StatusCode::UNAUTHORIZED)
}

fn truncate_chars(value: &str, limit: usize) -> String {
    if value.chars().count() <= limit {
        return value.to_owned();
    }
    let mut truncated: String = value.chars().take(limit.saturating_sub(1)).collect();
    truncated.push('…');
    truncated
}
