//! System-admin CRUD for non-secret AI endpoint profiles.

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use uuid::Uuid;

use super::access::require_admin;
use crate::{
    ai::{model_options::validate_model_option, AiModelOptionView, SaveAiModelOption},
    audit::AuditEventDraft,
    state::SharedState,
};

#[utoipa::path(
    get,
    path = "/api/admin/ai-models",
    responses((status = 200, description = "Configured AI endpoints", body = [AiModelOptionView]))
)]
pub async fn list(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> Result<Json<Vec<AiModelOptionView>>, StatusCode> {
    require_admin(&state, &headers).await?;
    state
        .ai_model_options()
        .await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

#[utoipa::path(
    post,
    path = "/api/admin/ai-models",
    request_body = SaveAiModelOption,
    responses(
        (status = 201, description = "AI endpoint created", body = AiModelOptionView),
        (status = 400, description = "Invalid endpoint configuration")
    )
)]
pub async fn create(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(payload): Json<SaveAiModelOption>,
) -> Result<(StatusCode, Json<AiModelOptionView>), StatusCode> {
    let actor = require_admin(&state, &headers).await?;
    if !validate_model_option(&payload) {
        return Err(StatusCode::BAD_REQUEST);
    }
    require_audit(
        &state,
        AuditEventDraft::system(&actor, "ai_model.create_requested").target_type("ai_model"),
    )
    .await?;
    state
        .create_ai_model_option(&payload)
        .await
        .map(|option| (StatusCode::CREATED, Json(option)))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

#[utoipa::path(
    put,
    path = "/api/admin/ai-models/{id}",
    params(("id" = Uuid, Path, description = "AI endpoint id")),
    request_body = SaveAiModelOption,
    responses(
        (status = 200, description = "AI endpoint updated", body = AiModelOptionView),
        (status = 400, description = "Invalid endpoint configuration"),
        (status = 404, description = "AI endpoint not found")
    )
)]
pub async fn update(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(payload): Json<SaveAiModelOption>,
) -> Result<Json<AiModelOptionView>, StatusCode> {
    let actor = require_admin(&state, &headers).await?;
    if id.is_nil() || !validate_model_option(&payload) {
        return Err(StatusCode::BAD_REQUEST);
    }
    require_audit(
        &state,
        AuditEventDraft::system(&actor, "ai_model.update_requested").target("ai_model", id),
    )
    .await?;
    state
        .update_ai_model_option(id, &payload)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

#[utoipa::path(
    delete,
    path = "/api/admin/ai-models/{id}",
    params(("id" = Uuid, Path, description = "AI endpoint id")),
    responses(
        (status = 204, description = "AI endpoint deleted"),
        (status = 400, description = "Environment endpoint cannot be deleted"),
        (status = 404, description = "AI endpoint not found")
    )
)]
pub async fn delete(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    let actor = require_admin(&state, &headers).await?;
    if id.is_nil() {
        return Err(StatusCode::BAD_REQUEST);
    }
    require_audit(
        &state,
        AuditEventDraft::system(&actor, "ai_model.delete_requested").target("ai_model", id),
    )
    .await?;
    state
        .delete_ai_model_option(id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .then_some(StatusCode::NO_CONTENT)
        .ok_or(StatusCode::NOT_FOUND)
}

async fn require_audit(state: &SharedState, draft: AuditEventDraft) -> Result<(), StatusCode> {
    state
        .record_audit_event(draft)
        .await
        .map(|_| ())
        .map_err(|error| {
            tracing::error!("required AI model audit failed: {error}");
            StatusCode::INTERNAL_SERVER_ERROR
        })
}
