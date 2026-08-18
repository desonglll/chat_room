//! The "magic wand" endpoint: summarize recent room activity and suggest replies.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use uuid::Uuid;

use crate::ai::{AiContextMessage, AiSuggestions};
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
    let Some(assistant) = state.ai_assistant() else {
        return Err(StatusCode::SERVICE_UNAVAILABLE);
    };

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
            sender: message.sender,
            content: message.content,
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
