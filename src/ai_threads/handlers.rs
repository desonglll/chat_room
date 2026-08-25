use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use uuid::Uuid;

use super::models::{AiThread, AiThreadMessage, CreateAiThreadRequest, UpdateAiThreadRequest};
use crate::models::User;
use crate::state::SharedState;
use crate::user_handlers::bearer_token;

pub(super) const DEFAULT_TITLE: &str = "新对话";
const MAX_TITLE_CHARS: usize = 80;

pub(super) async fn current_user(
    state: &SharedState,
    headers: &HeaderMap,
) -> Result<User, StatusCode> {
    let token = bearer_token(headers)?;
    state
        .session_user(token)
        .await
        .map_err(internal_error)?
        .ok_or(StatusCode::UNAUTHORIZED)
}

pub(super) async fn validate_room_access(
    state: &SharedState,
    user_id: Uuid,
    room_id: Option<Uuid>,
) -> Result<(), StatusCode> {
    let Some(room_id) = room_id else {
        return Ok(());
    };
    state
        .conversation_summary(user_id, room_id)
        .await
        .map_err(internal_error)?
        .ok_or(StatusCode::FORBIDDEN)
        .map(|_| ())
}

#[utoipa::path(
    get,
    path = "/api/ai/threads",
    responses((status = 200, description = "Current user's AI sessions", body = [AiThread]))
)]
pub async fn list_threads(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> Result<Json<Vec<AiThread>>, StatusCode> {
    let user = current_user(&state, &headers).await?;
    state
        .ai_threads(user.id)
        .await
        .map(Json)
        .map_err(internal_error)
}

#[utoipa::path(
    post,
    path = "/api/ai/threads",
    request_body = CreateAiThreadRequest,
    responses(
        (status = 201, description = "AI session created", body = AiThread),
        (status = 400, description = "Invalid title"),
        (status = 403, description = "Conversation is not accessible")
    )
)]
pub async fn create_thread(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(payload): Json<CreateAiThreadRequest>,
) -> Result<(StatusCode, Json<AiThread>), StatusCode> {
    let user = current_user(&state, &headers).await?;
    let title = payload.title.as_deref().unwrap_or(DEFAULT_TITLE).trim();
    if title.is_empty() || title.chars().count() > MAX_TITLE_CHARS {
        return Err(StatusCode::BAD_REQUEST);
    }
    validate_room_access(&state, user.id, payload.room_id).await?;
    state
        .create_ai_thread(
            user.id,
            title,
            payload.room_id,
            payload.thinking_enabled.unwrap_or(false),
        )
        .await
        .map(|thread| (StatusCode::CREATED, Json(thread)))
        .map_err(internal_error)
}

#[utoipa::path(
    patch,
    path = "/api/ai/threads/{id}",
    params(("id" = Uuid, Path, description = "AI session id")),
    request_body = UpdateAiThreadRequest,
    responses(
        (status = 200, description = "AI session updated", body = AiThread),
        (status = 404, description = "AI session not found")
    )
)]
pub async fn update_thread(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(thread_id): Path<Uuid>,
    Json(payload): Json<UpdateAiThreadRequest>,
) -> Result<Json<AiThread>, StatusCode> {
    let user = current_user(&state, &headers).await?;
    let current = state
        .ai_thread(user.id, thread_id)
        .await
        .map_err(internal_error)?
        .ok_or(StatusCode::NOT_FOUND)?;
    let title = payload.title.as_deref().unwrap_or(&current.title).trim();
    if title.is_empty() || title.chars().count() > MAX_TITLE_CHARS {
        return Err(StatusCode::BAD_REQUEST);
    }
    let room_id = if payload.clear_room {
        None
    } else {
        payload.room_id.or(current.room_id)
    };
    validate_room_access(&state, user.id, room_id).await?;
    state
        .update_ai_thread(
            user.id,
            thread_id,
            title,
            room_id,
            payload.thinking_enabled.unwrap_or(current.thinking_enabled),
        )
        .await
        .map_err(internal_error)?
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

#[utoipa::path(
    delete,
    path = "/api/ai/threads/{id}",
    params(("id" = Uuid, Path, description = "AI session id")),
    responses(
        (status = 204, description = "AI session deleted"),
        (status = 404, description = "AI session not found")
    )
)]
pub async fn delete_thread(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(thread_id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    let user = current_user(&state, &headers).await?;
    state
        .delete_ai_thread(user.id, thread_id)
        .await
        .map_err(internal_error)?
        .then_some(StatusCode::NO_CONTENT)
        .ok_or(StatusCode::NOT_FOUND)
}

#[utoipa::path(
    get,
    path = "/api/ai/threads/{id}/messages",
    params(("id" = Uuid, Path, description = "AI session id")),
    responses(
        (status = 200, description = "AI session messages", body = [AiThreadMessage]),
        (status = 404, description = "AI session not found")
    )
)]
pub async fn list_messages(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(thread_id): Path<Uuid>,
) -> Result<Json<Vec<AiThreadMessage>>, StatusCode> {
    let user = current_user(&state, &headers).await?;
    state
        .ai_thread_messages(user.id, thread_id, 500)
        .await
        .map_err(internal_error)?
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

pub(super) fn internal_error(error: impl std::fmt::Display) -> StatusCode {
    tracing::error!("AI thread operation failed: {error}");
    StatusCode::INTERNAL_SERVER_ERROR
}
