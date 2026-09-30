//! HTTP translation for the poll interface in `super`. No domain rules live here.

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use uuid::Uuid;

use super::model::{CreatePollRequest, PollError, PollVoterPage, VoteRequest, VotersQuery};
use crate::models::{PollState, StoredMessage, User};
use crate::state::SharedState;
use crate::user_handlers::bearer_token;

async fn caller(state: &SharedState, headers: &HeaderMap) -> Result<User, StatusCode> {
    state
        .session_user(bearer_token(headers)?)
        .await
        .map_err(|error| PollError::Database(error).status())?
        .ok_or(StatusCode::UNAUTHORIZED)
}

fn status(error: PollError) -> StatusCode {
    error.status()
}

#[utoipa::path(
    post,
    path = "/api/chats/{id}/polls",
    params(("id" = Uuid, Path, description = "Chat identifier")),
    request_body = CreatePollRequest,
    responses(
        (status = 201, description = "The poll message, with its `poll` field", body = StoredMessage),
        (status = 400, description = "Malformed poll"),
        (status = 401, description = "Missing session"),
        (status = 403, description = "The caller may not send messages in this chat"),
        (status = 404, description = "Chat not found or caller is not a member")
    )
)]
pub async fn create(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(room_id): Path<Uuid>,
    Json(request): Json<CreatePollRequest>,
) -> Result<(StatusCode, Json<StoredMessage>), StatusCode> {
    let user = caller(&state, &headers).await?;
    let message = super::create_poll(&state, room_id, &user, &request)
        .await
        .map_err(status)?;
    Ok((StatusCode::CREATED, Json(message)))
}

#[utoipa::path(
    get,
    path = "/api/polls/{message_id}",
    params(("message_id" = Uuid, Path, description = "The poll message")),
    responses(
        (status = 200, description = "The poll as the caller sees it", body = PollState),
        (status = 401, description = "Missing session"),
        (status = 404, description = "No such poll, or not visible to the caller")
    )
)]
pub async fn get(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(message_id): Path<Uuid>,
) -> Result<Json<PollState>, StatusCode> {
    let user = caller(&state, &headers).await?;
    super::poll_for_viewer(&state, message_id, user.id)
        .await
        .map(Json)
        .map_err(status)
}

#[utoipa::path(
    post,
    path = "/api/polls/{message_id}/votes",
    params(("message_id" = Uuid, Path, description = "The poll message")),
    request_body = VoteRequest,
    responses(
        (status = 200, description = "The poll after the vote, as the caller sees it", body = PollState),
        (status = 400, description = "Options do not fit this poll"),
        (status = 401, description = "Missing session"),
        (status = 404, description = "No such poll, or not visible to the caller"),
        (status = 409, description = "Poll closed, or quiz already answered")
    )
)]
pub async fn vote(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(message_id): Path<Uuid>,
    Json(request): Json<VoteRequest>,
) -> Result<Json<PollState>, StatusCode> {
    let user = caller(&state, &headers).await?;
    super::cast_vote(&state, message_id, user.id, &request.options)
        .await
        .map(Json)
        .map_err(status)
}

#[utoipa::path(
    delete,
    path = "/api/polls/{message_id}/votes",
    params(("message_id" = Uuid, Path, description = "The poll message")),
    responses(
        (status = 200, description = "The poll after the retraction", body = PollState),
        (status = 401, description = "Missing session"),
        (status = 404, description = "No such poll, or not visible to the caller"),
        (status = 409, description = "Poll closed, or a quiz (answers are final)")
    )
)]
pub async fn retract(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(message_id): Path<Uuid>,
) -> Result<Json<PollState>, StatusCode> {
    let user = caller(&state, &headers).await?;
    super::retract_vote(&state, message_id, user.id)
        .await
        .map(Json)
        .map_err(status)
}

#[utoipa::path(
    post,
    path = "/api/polls/{message_id}/close",
    params(("message_id" = Uuid, Path, description = "The poll message")),
    responses(
        (status = 200, description = "The closed poll", body = PollState),
        (status = 401, description = "Missing session"),
        (status = 403, description = "Neither the poll's author nor a chat administrator"),
        (status = 404, description = "No such poll, or not visible to the caller")
    )
)]
pub async fn close(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(message_id): Path<Uuid>,
) -> Result<Json<PollState>, StatusCode> {
    let user = caller(&state, &headers).await?;
    super::close_poll(&state, message_id, user.id)
        .await
        .map(Json)
        .map_err(status)
}

#[utoipa::path(
    get,
    path = "/api/polls/{message_id}/voters",
    params(
        ("message_id" = Uuid, Path, description = "The poll message"),
        ("option" = u32, Query, description = "Option index"),
        ("limit" = Option<i64>, Query, description = "Page size (1-100, default 50)"),
        ("offset" = Option<i64>, Query, description = "Rows to skip")
    ),
    responses(
        (status = 200, description = "One page of the option's voters, newest first", body = PollVoterPage),
        (status = 400, description = "Option index out of range"),
        (status = 401, description = "Missing session"),
        (status = 403, description = "Anonymous poll: voters are never listed"),
        (status = 404, description = "No such poll, or not visible to the caller")
    )
)]
pub async fn voters(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(message_id): Path<Uuid>,
    Query(query): Query<VotersQuery>,
) -> Result<Json<PollVoterPage>, StatusCode> {
    let user = caller(&state, &headers).await?;
    super::list_voters(
        &state,
        message_id,
        user.id,
        query.option,
        query.limit,
        query.offset,
    )
    .await
    .map(Json)
    .map_err(status)
}
