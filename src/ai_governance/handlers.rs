use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use chrono::{Duration, Utc};
use uuid::Uuid;

use super::{
    models::AiUsageQuery, AiGovernanceSettings, AiUsageReport, RoomAiPolicy,
    UpdateAiGovernanceSettings, UpdateRoomAiPolicy,
};
use crate::{
    admin::access::require_admin, models::User, state::SharedState, user_handlers::bearer_token,
};

async fn actor(state: &SharedState, headers: &HeaderMap) -> Result<User, StatusCode> {
    state
        .session_user(bearer_token(headers)?)
        .await
        .map_err(internal_error)?
        .ok_or(StatusCode::UNAUTHORIZED)
}

async fn active_role(
    state: &SharedState,
    room_id: Uuid,
    user_id: Uuid,
) -> Result<String, StatusCode> {
    let (status, role) = state
        .membership_identity(room_id, user_id)
        .await
        .map_err(internal_error)?
        .ok_or(StatusCode::FORBIDDEN)?;
    (status == "active")
        .then_some(role)
        .ok_or(StatusCode::FORBIDDEN)
}

#[utoipa::path(
    get,
    path = "/api/rooms/{id}/ai-policy",
    params(("id" = Uuid, Path, description = "Room identifier")),
    responses(
        (status = 200, description = "Room AI policy", body = RoomAiPolicy),
        (status = 403, description = "Active room membership required")
    )
)]
pub async fn room_policy(
    State(state): State<SharedState>,
    Path(room_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<RoomAiPolicy>, StatusCode> {
    let user = actor(&state, &headers).await?;
    active_role(&state, room_id, user.id).await?;
    state
        .room_ai_policy(room_id)
        .await
        .map(Json)
        .map_err(internal_error)
}

#[utoipa::path(
    patch,
    path = "/api/rooms/{id}/ai-policy",
    params(("id" = Uuid, Path, description = "Room identifier")),
    request_body = UpdateRoomAiPolicy,
    responses(
        (status = 200, description = "Updated Room AI policy", body = RoomAiPolicy),
        (status = 400, description = "Invalid policy"),
        (status = 403, description = "Room owner required"),
        (status = 409, description = "Policy version changed")
    )
)]
pub async fn update_room_policy(
    State(state): State<SharedState>,
    Path(room_id): Path<Uuid>,
    headers: HeaderMap,
    Json(payload): Json<UpdateRoomAiPolicy>,
) -> Result<Json<RoomAiPolicy>, StatusCode> {
    let user = actor(&state, &headers).await?;
    if active_role(&state, room_id, user.id).await? != "owner" {
        return Err(StatusCode::FORBIDDEN);
    }
    if !matches!(payload.mode.as_str(), "disabled" | "members" | "admins") || payload.version < 0 {
        return Err(StatusCode::BAD_REQUEST);
    }
    state
        .update_room_ai_policy(room_id, user.id, &payload.mode, payload.version)
        .await
        .map_err(internal_error)?
        .map(Json)
        .ok_or(StatusCode::CONFLICT)
}

#[utoipa::path(
    get,
    path = "/api/admin/ai-governance",
    responses((status = 200, description = "Deployment AI governance", body = AiGovernanceSettings))
)]
pub async fn admin_settings(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> Result<Json<AiGovernanceSettings>, StatusCode> {
    require_admin(&state, &headers).await?;
    state
        .ai_governance_settings()
        .await
        .map(Json)
        .map_err(internal_error)
}

#[utoipa::path(
    patch,
    path = "/api/admin/ai-governance",
    request_body = UpdateAiGovernanceSettings,
    responses(
        (status = 200, description = "Updated deployment AI governance", body = AiGovernanceSettings),
        (status = 400, description = "Invalid limits, model, or pricing")
    )
)]
pub async fn update_admin_settings(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(payload): Json<UpdateAiGovernanceSettings>,
) -> Result<Json<AiGovernanceSettings>, StatusCode> {
    let admin = require_admin(&state, &headers).await?;
    if !state
        .save_ai_governance_settings(admin.id, &payload)
        .await
        .map_err(internal_error)?
    {
        return Err(StatusCode::BAD_REQUEST);
    }
    state
        .ai_governance_settings()
        .await
        .map(Json)
        .map_err(internal_error)
}

#[utoipa::path(
    get,
    path = "/api/admin/ai-usage",
    params(AiUsageQuery),
    responses((status = 200, description = "Privacy-safe aggregate AI usage", body = AiUsageReport))
)]
pub async fn admin_usage(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Query(query): Query<AiUsageQuery>,
) -> Result<Json<AiUsageReport>, StatusCode> {
    require_admin(&state, &headers).await?;
    let group_by = query.group_by.as_deref().unwrap_or("room");
    if !matches!(group_by, "room" | "model") {
        return Err(StatusCode::BAD_REQUEST);
    }
    let to = query.to.unwrap_or_else(Utc::now);
    let from = query.from.unwrap_or(to - Duration::days(30));
    if from >= to || to - from > Duration::days(366) {
        return Err(StatusCode::BAD_REQUEST);
    }
    state
        .ai_usage_report(group_by, from, to)
        .await
        .map(Json)
        .map_err(internal_error)
}

fn internal_error(error: sqlx::Error) -> StatusCode {
    tracing::error!("AI governance database operation failed: {error}");
    StatusCode::INTERNAL_SERVER_ERROR
}
