use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use chrono::{Duration, Utc};
use uuid::Uuid;

use super::{
    confirmation::CandidateMutation,
    models::{
        AiExtractionCandidate, AiExtractionRun, CreateAiExtractionRequest,
        UpdateAiExtractionCandidateRequest,
    },
    runner::spawn_run,
};
use crate::{
    ai_handlers::require_room_password, models::User, state::SharedState,
    user_handlers::bearer_token,
};

async fn actor(state: &SharedState, headers: &HeaderMap) -> Result<User, StatusCode> {
    state
        .session_user(bearer_token(headers)?)
        .await
        .map_err(internal_error)?
        .ok_or(StatusCode::UNAUTHORIZED)
}

async fn authorize_room(
    state: &SharedState,
    user_id: Uuid,
    room_id: Uuid,
    headers: &HeaderMap,
) -> Result<(), StatusCode> {
    let room = state.room(room_id).await.ok_or(StatusCode::NOT_FOUND)?;
    require_room_password(&room, headers)?;
    let membership = state
        .room_membership(room_id, user_id)
        .await
        .map_err(internal_error)?
        .ok_or(StatusCode::FORBIDDEN)?;
    (membership.status == "active")
        .then_some(())
        .ok_or(StatusCode::FORBIDDEN)
}

#[utoipa::path(
    post,
    path = "/api/rooms/{room_id}/ai/extractions",
    params(("room_id" = Uuid, Path, description = "Room identifier")),
    request_body = CreateAiExtractionRequest,
    responses(
        (status = 202, description = "Durable extraction accepted", body = AiExtractionRun),
        (status = 400, description = "Invalid time range"),
        (status = 401, description = "Missing session or incorrect room password"),
        (status = 403, description = "Account is not an active room member"),
        (status = 404, description = "Room not found"),
        (status = 409, description = "Idempotency key belongs to another room"),
        (status = 503, description = "AI model unavailable")
    )
)]
pub async fn create(
    State(state): State<SharedState>,
    Path(room_id): Path<Uuid>,
    headers: HeaderMap,
    Json(payload): Json<CreateAiExtractionRequest>,
) -> Result<(StatusCode, Json<AiExtractionRun>), StatusCode> {
    let user = actor(&state, &headers).await?;
    authorize_room(&state, user.id, room_id, &headers).await?;
    if payload.client_request_id.is_nil()
        || payload.from_at >= payload.to_at
        || payload.to_at - payload.from_at > Duration::days(365)
        || payload.to_at > Utc::now() + Duration::minutes(5)
    {
        return Err(StatusCode::BAD_REQUEST);
    }
    let model = state
        .resolve_ai_model(payload.model_option_id, false)
        .await
        .map_err(internal_error)?
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    let (run, created) = state
        .create_extraction_run(
            user.id,
            room_id,
            payload.from_at,
            payload.to_at,
            payload.client_request_id,
            &model,
        )
        .await
        .map_err(internal_error)?;
    if run.room_id != room_id {
        return Err(StatusCode::CONFLICT);
    }
    if created {
        spawn_run(state, run.id);
    }
    Ok((StatusCode::ACCEPTED, Json(run)))
}

#[utoipa::path(
    get,
    path = "/api/ai/extractions/{id}",
    params(("id" = Uuid, Path, description = "Extraction run identifier")),
    responses(
        (status = 200, description = "Extraction run and candidates", body = AiExtractionRun),
        (status = 401, description = "Missing session or incorrect room password"),
        (status = 403, description = "Account is not an active room member"),
        (status = 404, description = "Extraction run not found")
    )
)]
pub async fn get(
    State(state): State<SharedState>,
    Path(run_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<AiExtractionRun>, StatusCode> {
    let user = actor(&state, &headers).await?;
    let room_id = state
        .extraction_run_room(user.id, run_id)
        .await
        .map_err(internal_error)?
        .ok_or(StatusCode::NOT_FOUND)?;
    authorize_room(&state, user.id, room_id, &headers).await?;
    state
        .ai_extraction_run(user.id, run_id)
        .await
        .map_err(internal_error)?
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

#[utoipa::path(
    patch,
    path = "/api/ai/extraction-candidates/{id}",
    params(("id" = Uuid, Path, description = "Extraction candidate identifier")),
    request_body = UpdateAiExtractionCandidateRequest,
    responses(
        (status = 200, description = "Candidate status and persisted result", body = AiExtractionCandidate),
        (status = 400, description = "Invalid action or version"),
        (status = 401, description = "Missing session or incorrect room password"),
        (status = 403, description = "Account is not an active room member"),
        (status = 404, description = "Candidate not found"),
        (status = 409, description = "Candidate changed concurrently")
    )
)]
pub async fn update_candidate(
    State(state): State<SharedState>,
    Path(candidate_id): Path<Uuid>,
    headers: HeaderMap,
    Json(payload): Json<UpdateAiExtractionCandidateRequest>,
) -> Result<Json<AiExtractionCandidate>, StatusCode> {
    if !matches!(payload.action.as_str(), "confirm" | "dismiss") || payload.version < 1 {
        return Err(StatusCode::BAD_REQUEST);
    }
    let user = actor(&state, &headers).await?;
    let room_id = state
        .extraction_candidate_room(user.id, candidate_id)
        .await
        .map_err(internal_error)?
        .ok_or(StatusCode::NOT_FOUND)?;
    authorize_room(&state, user.id, room_id, &headers).await?;
    match state
        .update_extraction_candidate(user.id, candidate_id, &payload.action, payload.version)
        .await
        .map_err(internal_error)?
    {
        CandidateMutation::Applied(candidate) => Ok(Json(*candidate)),
        CandidateMutation::NotFound => Err(StatusCode::NOT_FOUND),
        CandidateMutation::Conflict => Err(StatusCode::CONFLICT),
    }
}

fn internal_error(_error: sqlx::Error) -> StatusCode {
    tracing::error!("AI extraction database operation failed");
    StatusCode::INTERNAL_SERVER_ERROR
}
