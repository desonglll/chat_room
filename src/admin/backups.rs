//! Administrator backup schedule, export, validation, and restore routes.

use axum::{
    extract::{Multipart, Request, State},
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use utoipa::ToSchema;

use super::{access::require_admin, backup_http::internal_error};
pub use super::{
    backup_http::BackupApiError,
    backup_restore::{RestoreBackupResult, RestoreValidationResult},
};
use crate::{audit::AuditEventDraft, backup, state::SharedState};

pub fn routes() -> Router<SharedState> {
    Router::new()
        .route("/api/admin/backups", get(get_status))
        .route("/api/admin/backups/run", post(run_now))
        .route("/api/admin/backups/export", post(export))
        .route(
            "/api/admin/backups/restore",
            post(restore).layer(axum::extract::DefaultBodyLimit::disable()),
        )
        .route(
            "/api/admin/backups/restore/execute",
            post(execute_restore).layer(axum::extract::DefaultBodyLimit::disable()),
        )
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ExportBackupRequest {
    #[serde(default)]
    pub(super) include_files: bool,
}

#[utoipa::path(
    get,
    path = "/api/admin/backups",
    responses((status = 200, description = "Backup schedule and recent runs", body = backup::BackupStatus))
)]
pub async fn get_status(State(state): State<SharedState>, headers: HeaderMap) -> Response {
    if let Err(status) = require_admin(&state, &headers).await {
        return status.into_response();
    }
    match backup::status(&state).await {
        Ok(status) => Json(status).into_response(),
        Err(error) => internal_error("读取备份状态", error),
    }
}

#[utoipa::path(
    post,
    path = "/api/admin/backups/run",
    request_body = ExportBackupRequest,
    responses((status = 200, description = "Completed manual backup", body = backup::BackupRun))
)]
pub async fn run_now(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(request): Json<ExportBackupRequest>,
) -> Response {
    let actor = match require_admin(&state, &headers).await {
        Ok(actor) => actor,
        Err(status) => return status.into_response(),
    };
    if let Err(error) = state
        .record_audit_event(
            AuditEventDraft::system(&actor, "backup.run_requested")
                .target_type("backup")
                .detail("include_files", request.include_files),
        )
        .await
    {
        return internal_error("记录备份运行审计", error.into());
    }
    match backup::run_backup(&state, "manual", request.include_files).await {
        Ok(run) => Json(run).into_response(),
        Err(error) => internal_error("运行备份", error),
    }
}

#[utoipa::path(
    post,
    path = "/api/admin/backups/export",
    request_body = ExportBackupRequest,
    responses((status = 200, description = "Verified database backup archive"))
)]
pub async fn export(
    state: State<SharedState>,
    headers: HeaderMap,
    request: Json<ExportBackupRequest>,
) -> Response {
    super::backup_export::export(state, headers, request).await
}

#[utoipa::path(
    post,
    path = "/api/admin/backups/restore",
    request_body(content = String, content_type = "multipart/form-data"),
    responses((status = 200, description = "Validated backup without changing data", body = RestoreValidationResult))
)]
pub async fn restore(
    state: State<SharedState>,
    headers: HeaderMap,
    multipart: Multipart,
) -> Response {
    super::backup_restore::validate(state, headers, multipart).await
}

#[utoipa::path(
    post,
    path = "/api/admin/backups/restore/execute",
    request_body(content = String, content_type = "multipart/form-data"),
    responses((status = 200, description = "Confirmed backup restore", body = RestoreBackupResult))
)]
pub async fn execute_restore(
    state: State<SharedState>,
    headers: HeaderMap,
    multipart: Multipart,
) -> Response {
    super::backup_restore::execute(state, headers, multipart).await
}

pub async fn reject_during_restore(
    State(state): State<SharedState>,
    request: Request,
    next: Next,
) -> Response {
    let path = request.uri().path();
    if path == "/api/config" {
        return next.run(request).await;
    }
    if state.maintenance_active() {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    }
    if path.starts_with("/api/admin/backups") {
        return next.run(request).await;
    }
    let _request_guard = state.lock_request().await;
    if state.maintenance_active() {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    }
    next.run(request).await
}
