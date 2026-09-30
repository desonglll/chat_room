//! Paginated history for one chat.
//!
//! Split out of `handlers` for the same reason as `lifecycle_handlers`.

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use chrono::Utc;
use serde::Deserialize;
use uuid::Uuid;

use super::handlers::authorize_chat;
use crate::message_store::MessageCursor;
use crate::models::StoredMessage;
use crate::state::{with_pool, SharedState};
use crate::user_handlers::bearer_token;

#[derive(Deserialize)]
pub struct MessageHistoryQuery {
    pub limit: Option<i64>,
    /// Exclusive cursor: return messages strictly older than this message id.
    pub before: Option<Uuid>,
}

/// Return persisted chat messages in reverse-chronological pages (newest first,
/// or strictly older than `before`) for history backfill.
#[utoipa::path(
    get,
    path = "/api/chats/{id}/messages",
    params(
        ("id" = Uuid, description = "Chat id"),
        ("limit" = Option<i64>, Query, description = "Messages to return (1-500)"),
        ("before" = Option<Uuid>, Query, description = "Exclusive message cursor for backfilling older history"),
        ("x-room-password" = Option<String>, Header, description = "Required for private chats")
    ),
    responses(
        (status = 200, description = "Persisted chat messages", body = Vec<StoredMessage>),
        (status = 400, description = "Unknown `before` cursor"),
        (status = 401, description = "Missing or incorrect chat password"),
        (status = 404, description = "Chat not found"),
        (status = 500, description = "Database error")
    )
)]
pub async fn list_messages(
    State(state): State<SharedState>,
    Path(id): Path<Uuid>,
    Query(query): Query<MessageHistoryQuery>,
    headers: HeaderMap,
) -> Result<Json<Vec<StoredMessage>>, StatusCode> {
    let chat = state.chat(id).await.ok_or(StatusCode::NOT_FOUND)?;
    let token = bearer_token(&headers)?;
    let user = state
        .session_user(token)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::UNAUTHORIZED)?;
    // TG-201: reading needs an active membership, not the right to send — a muted member
    // still reads.
    if !state
        .can_read_chat(id, user.id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    {
        return Err(StatusCode::FORBIDDEN);
    }
    if chat.has_password {
        let supplied = headers
            .get("x-room-password")
            .and_then(|value| value.to_str().ok());
        let Some(supplied) = supplied else {
            return Err(StatusCode::UNAUTHORIZED);
        };
        if !authorize_chat(&chat, Some(supplied)) {
            return Err(StatusCode::UNAUTHORIZED);
        }
    }

    let before = match query.before {
        Some(message_id) => {
            let created_at = with_pool!(state, |pool| {
                sqlx::query_scalar::<_, chrono::DateTime<Utc>>(
                    "SELECT created_at FROM messages WHERE id = $1 AND room_id = $2",
                )
                .bind(message_id)
                .bind(id)
                .fetch_optional(pool)
                .await
            })
            .map_err(|error| {
                tracing::error!("resolve message cursor failed: {}", error);
                StatusCode::INTERNAL_SERVER_ERROR
            })?
            .ok_or(StatusCode::BAD_REQUEST)?;
            Some(MessageCursor {
                created_at,
                id: message_id,
            })
        }
        None => None,
    };

    match state
        .message_history(
            id,
            query.limit.unwrap_or(100),
            before.as_ref(),
            Some(user.id),
        )
        .await
    {
        Ok(messages) => Ok(Json(messages)),
        Err(error) => {
            tracing::error!("load chat message history failed: {}", error);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}
