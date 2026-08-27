use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use uuid::Uuid;

use super::models::{CreateRoomTaskRequest, RoomTask, TaskMutation, UpdateRoomTaskRequest};
use crate::{
    ai_handlers::require_room_password, models::User, state::SharedState,
    user_handlers::bearer_token,
};

async fn task_actor(
    state: &SharedState,
    room_id: Uuid,
    headers: &HeaderMap,
) -> Result<User, StatusCode> {
    let room = state.room(room_id).await.ok_or(StatusCode::NOT_FOUND)?;
    require_room_password(&room, headers)?;
    state
        .session_user(bearer_token(headers)?)
        .await
        .map_err(internal_error)?
        .ok_or(StatusCode::UNAUTHORIZED)
}

#[utoipa::path(
    get,
    path = "/api/rooms/{room_id}/tasks",
    params(("room_id" = Uuid, Path, description = "Room identifier")),
    responses(
        (status = 200, description = "Tasks visible to the active room member", body = [RoomTask]),
        (status = 401, description = "Missing session or incorrect room password"),
        (status = 403, description = "Account is not an active room member"),
        (status = 404, description = "Room not found")
    )
)]
pub async fn list(
    State(state): State<SharedState>,
    Path(room_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Vec<RoomTask>>, StatusCode> {
    let actor = task_actor(&state, room_id, &headers).await?;
    state
        .room_tasks(room_id, actor.id)
        .await
        .map_err(internal_error)?
        .map(Json)
        .ok_or(StatusCode::FORBIDDEN)
}

#[utoipa::path(
    post,
    path = "/api/rooms/{room_id}/tasks",
    params(("room_id" = Uuid, Path, description = "Room identifier")),
    request_body = CreateRoomTaskRequest,
    responses(
        (status = 201, description = "Task created", body = RoomTask),
        (status = 400, description = "Invalid title, assignee, due date, or source message"),
        (status = 422, description = "Request body could not be parsed"),
        (status = 401, description = "Missing session or incorrect room password"),
        (status = 403, description = "Account is not an active room member"),
        (status = 404, description = "Room not found")
    )
)]
pub async fn create(
    State(state): State<SharedState>,
    Path(room_id): Path<Uuid>,
    headers: HeaderMap,
    Json(payload): Json<CreateRoomTaskRequest>,
) -> Result<(StatusCode, Json<RoomTask>), StatusCode> {
    let actor = task_actor(&state, room_id, &headers).await?;
    let actor_name = if actor.display_name.trim().is_empty() {
        actor.username.as_str()
    } else {
        actor.display_name.trim()
    };
    match state
        .create_room_task(room_id, actor.id, actor_name, payload)
        .await
        .map_err(internal_error)?
    {
        TaskMutation::Applied(task) => Ok((StatusCode::CREATED, Json(task))),
        TaskMutation::Forbidden => Err(StatusCode::FORBIDDEN),
        TaskMutation::InvalidAssignee
        | TaskMutation::InvalidSource
        | TaskMutation::InvalidValue => Err(StatusCode::BAD_REQUEST),
        TaskMutation::NotFound => Err(StatusCode::NOT_FOUND),
        TaskMutation::Conflict => Err(StatusCode::CONFLICT),
    }
}

#[utoipa::path(
    patch,
    path = "/api/rooms/{room_id}/tasks/{task_id}",
    params(
        ("room_id" = Uuid, Path, description = "Room identifier"),
        ("task_id" = Uuid, Path, description = "Task identifier")
    ),
    request_body = UpdateRoomTaskRequest,
    responses(
        (status = 200, description = "Task updated", body = RoomTask),
        (status = 400, description = "Invalid title, status, assignee, or version"),
        (status = 422, description = "Request body could not be parsed"),
        (status = 401, description = "Missing session or incorrect room password"),
        (status = 403, description = "Account cannot update this task"),
        (status = 404, description = "Room or task not found"),
        (status = 409, description = "Task was updated by another account")
    )
)]
pub async fn update(
    State(state): State<SharedState>,
    Path((room_id, task_id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
    Json(payload): Json<UpdateRoomTaskRequest>,
) -> Result<Json<RoomTask>, StatusCode> {
    let actor = task_actor(&state, room_id, &headers).await?;
    match state
        .update_room_task(room_id, task_id, actor.id, payload)
        .await
        .map_err(internal_error)?
    {
        TaskMutation::Applied(task) => Ok(Json(task)),
        TaskMutation::NotFound => Err(StatusCode::NOT_FOUND),
        TaskMutation::Forbidden => Err(StatusCode::FORBIDDEN),
        TaskMutation::Conflict => Err(StatusCode::CONFLICT),
        TaskMutation::InvalidAssignee
        | TaskMutation::InvalidSource
        | TaskMutation::InvalidValue => Err(StatusCode::BAD_REQUEST),
    }
}

#[utoipa::path(
    delete,
    path = "/api/rooms/{room_id}/tasks/{task_id}",
    params(
        ("room_id" = Uuid, Path, description = "Room identifier"),
        ("task_id" = Uuid, Path, description = "Task identifier")
    ),
    responses(
        (status = 204, description = "Task deleted"),
        (status = 401, description = "Missing session or incorrect room password"),
        (status = 403, description = "Only an active room owner or admin can delete tasks"),
        (status = 404, description = "Room or task not found")
    )
)]
pub async fn delete(
    State(state): State<SharedState>,
    Path((room_id, task_id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
) -> Result<StatusCode, StatusCode> {
    let actor = task_actor(&state, room_id, &headers).await?;
    match state
        .delete_room_task(room_id, task_id, actor.id)
        .await
        .map_err(internal_error)?
    {
        TaskMutation::Applied(()) => Ok(StatusCode::NO_CONTENT),
        TaskMutation::NotFound => Err(StatusCode::NOT_FOUND),
        TaskMutation::Forbidden => Err(StatusCode::FORBIDDEN),
        TaskMutation::Conflict => Err(StatusCode::CONFLICT),
        TaskMutation::InvalidAssignee
        | TaskMutation::InvalidSource
        | TaskMutation::InvalidValue => Err(StatusCode::BAD_REQUEST),
    }
}

fn internal_error(error: sqlx::Error) -> StatusCode {
    tracing::error!("room task operation failed: {error}");
    StatusCode::INTERNAL_SERVER_ERROR
}
