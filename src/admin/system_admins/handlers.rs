use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{admin::access::require_admin, state::SharedState};

use super::{AdminRoleError, SystemAdminView};

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateRegistrationInviteRequest {
    pub lifetime_hours: Option<i64>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct RegistrationInviteSecret {
    pub token: String,
    pub expires_at: DateTime<Utc>,
}

#[utoipa::path(
    get,
    path = "/api/admin/system-admins",
    responses(
        (status = 200, description = "Persistent system administrators", body = [SystemAdminView]),
        (status = 401, description = "Missing or expired session"),
        (status = 403, description = "Account is not a system administrator")
    )
)]
pub async fn list(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> Result<Json<Vec<SystemAdminView>>, StatusCode> {
    require_admin(&state, &headers).await?;
    state.list_system_admins().await.map(Json).map_err(|error| {
        tracing::error!("list system administrators failed: {error}");
        StatusCode::INTERNAL_SERVER_ERROR
    })
}

#[utoipa::path(
    put,
    path = "/api/admin/system-admins/{user_id}",
    params(("user_id" = Uuid, Path, description = "Account to grant")),
    responses(
        (status = 200, description = "Administrator granted", body = SystemAdminView),
        (status = 404, description = "Account does not exist")
    )
)]
pub async fn grant(
    State(state): State<SharedState>,
    Path(user_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<SystemAdminView>, StatusCode> {
    let actor = require_admin(&state, &headers).await?;
    state
        .grant_system_admin(actor.id, user_id)
        .await
        .map(Json)
        .map_err(map_role_error)
}

#[utoipa::path(
    delete,
    path = "/api/admin/system-admins/{user_id}",
    params(("user_id" = Uuid, Path, description = "Administrator to revoke")),
    responses(
        (status = 204, description = "Administrator revoked"),
        (status = 404, description = "Administrator does not exist"),
        (status = 409, description = "The last administrator cannot be revoked")
    )
)]
pub async fn revoke(
    State(state): State<SharedState>,
    Path(user_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<StatusCode, StatusCode> {
    let actor = require_admin(&state, &headers).await?;
    state
        .revoke_system_admin(actor.id, user_id)
        .await
        .map(|()| StatusCode::NO_CONTENT)
        .map_err(map_role_error)
}

#[utoipa::path(
    post,
    path = "/api/admin/registration-invites",
    request_body = CreateRegistrationInviteRequest,
    responses(
        (status = 201, description = "One-time invitation created", body = RegistrationInviteSecret),
        (status = 400, description = "Invalid lifetime")
    )
)]
pub async fn create_invite(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(request): Json<CreateRegistrationInviteRequest>,
) -> Result<(StatusCode, Json<RegistrationInviteSecret>), StatusCode> {
    let actor = require_admin(&state, &headers).await?;
    let lifetime_hours = request.lifetime_hours.unwrap_or(72);
    if !(1..=720).contains(&lifetime_hours) {
        return Err(StatusCode::BAD_REQUEST);
    }
    state
        .create_registration_invite(actor.id, lifetime_hours)
        .await
        .map(|(token, expires_at)| {
            (
                StatusCode::CREATED,
                Json(RegistrationInviteSecret { token, expires_at }),
            )
        })
        .map_err(map_role_error)
}

fn map_role_error(error: AdminRoleError) -> StatusCode {
    match error {
        AdminRoleError::UserNotFound | AdminRoleError::AdministratorNotFound => {
            StatusCode::NOT_FOUND
        }
        AdminRoleError::Forbidden => StatusCode::FORBIDDEN,
        AdminRoleError::LastAdministrator | AdminRoleError::BootstrapUnavailable => {
            StatusCode::CONFLICT
        }
        AdminRoleError::Database(error) => {
            tracing::error!("update system administrator failed: {error}");
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}
