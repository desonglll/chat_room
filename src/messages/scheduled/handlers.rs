//! HTTP translation for the scheduled-message interface in `super`. No domain rules live here.

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use uuid::Uuid;

use super::model::{
    CreateScheduledMessageRequest, ScheduledError, ScheduledMessage, UpdateScheduledMessageRequest,
};
use crate::models::{StoredMessage, User};
use crate::state::SharedState;
use crate::user_handlers::bearer_token;

async fn caller(state: &SharedState, headers: &HeaderMap) -> Result<User, StatusCode> {
    state
        .session_user(bearer_token(headers)?)
        .await
        .map_err(|error| ScheduledError::Database(error).status())?
        .ok_or(StatusCode::UNAUTHORIZED)
}

fn status(error: ScheduledError) -> StatusCode {
    error.status()
}

#[utoipa::path(
    get,
    path = "/api/chats/{id}/scheduled-messages",
    params(("id" = Uuid, Path, description = "Chat identifier")),
    responses(
        (status = 200, description = "The caller's own scheduled messages, soonest first", body = Vec<ScheduledMessage>),
        (status = 401, description = "Missing session"),
        (status = 404, description = "Chat not found or caller is not a member")
    )
)]
pub async fn list(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(room_id): Path<Uuid>,
) -> Result<Json<Vec<ScheduledMessage>>, StatusCode> {
    let user = caller(&state, &headers).await?;
    super::list(&state, room_id, user.id)
        .await
        .map(Json)
        .map_err(status)
}

#[utoipa::path(
    post,
    path = "/api/chats/{id}/scheduled-messages",
    params(("id" = Uuid, Path, description = "Chat identifier")),
    request_body = CreateScheduledMessageRequest,
    responses(
        (status = 201, description = "The scheduled message", body = ScheduledMessage),
        (status = 400, description = "Empty text, or a date in the past or over a year ahead"),
        (status = 401, description = "Missing session"),
        (status = 403, description = "The caller may not send messages in this chat"),
        (status = 404, description = "Chat not found or caller is not a member"),
        (status = 409, description = "100 scheduled messages are already pending in this chat")
    )
)]
pub async fn create(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(room_id): Path<Uuid>,
    Json(request): Json<CreateScheduledMessageRequest>,
) -> Result<(StatusCode, Json<ScheduledMessage>), StatusCode> {
    let user = caller(&state, &headers).await?;
    let scheduled = super::schedule(&state, room_id, &user, request)
        .await
        .map_err(status)?;
    Ok((StatusCode::CREATED, Json(scheduled)))
}

#[utoipa::path(
    patch,
    path = "/api/chats/{id}/scheduled-messages/{scheduled_id}",
    params(
        ("id" = Uuid, Path, description = "Chat identifier"),
        ("scheduled_id" = Uuid, Path, description = "Scheduled message identifier")
    ),
    request_body = UpdateScheduledMessageRequest,
    responses(
        (status = 200, description = "The updated scheduled message", body = ScheduledMessage),
        (status = 400, description = "Empty text, or an invalid date"),
        (status = 401, description = "Missing session"),
        (status = 403, description = "The caller may no longer send messages in this chat"),
        (status = 404, description = "Not found, not the caller's, or already delivered")
    )
)]
pub async fn update(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path((room_id, id)): Path<(Uuid, Uuid)>,
    Json(request): Json<UpdateScheduledMessageRequest>,
) -> Result<Json<ScheduledMessage>, StatusCode> {
    let user = caller(&state, &headers).await?;
    super::update(&state, room_id, id, &user, request)
        .await
        .map(Json)
        .map_err(status)
}

#[utoipa::path(
    delete,
    path = "/api/chats/{id}/scheduled-messages/{scheduled_id}",
    params(
        ("id" = Uuid, Path, description = "Chat identifier"),
        ("scheduled_id" = Uuid, Path, description = "Scheduled message identifier")
    ),
    responses(
        (status = 204, description = "Cancelled"),
        (status = 401, description = "Missing session"),
        (status = 404, description = "Not found, not the caller's, or already delivered")
    )
)]
pub async fn delete(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path((room_id, id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, StatusCode> {
    let user = caller(&state, &headers).await?;
    super::cancel(&state, room_id, id, user.id)
        .await
        .map(|()| StatusCode::NO_CONTENT)
        .map_err(status)
}

#[utoipa::path(
    post,
    path = "/api/chats/{id}/scheduled-messages/{scheduled_id}/send-now",
    params(
        ("id" = Uuid, Path, description = "Chat identifier"),
        ("scheduled_id" = Uuid, Path, description = "Scheduled message identifier")
    ),
    responses(
        (status = 200, description = "The delivered message (same id as the scheduled one)", body = StoredMessage),
        (status = 401, description = "Missing session"),
        (status = 403, description = "The caller may no longer send messages in this chat"),
        (status = 404, description = "Not found, not the caller's, or already delivered"),
        (status = 503, description = "Message write queue saturated; retry")
    )
)]
pub async fn send_now(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path((room_id, id)): Path<(Uuid, Uuid)>,
) -> Result<Json<StoredMessage>, StatusCode> {
    let user = caller(&state, &headers).await?;
    super::send_now(&state, room_id, id, &user)
        .await
        .map(Json)
        .map_err(status)
}
