//! Authentication and persistent authorization for system administration.

use axum::http::{HeaderMap, StatusCode};

use crate::{models::User, state::AppState, user_handlers::bearer_token};

pub(crate) async fn require_admin(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<User, StatusCode> {
    let token = bearer_token(headers)?;
    let user = state
        .session_user(token)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::UNAUTHORIZED)?;
    state
        .is_system_admin(user.id)
        .await
        .map_err(|error| {
            tracing::error!("check system administrator failed: {error}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .then_some(user)
        .ok_or(StatusCode::FORBIDDEN)
}

#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct AdminAccess {
    pub is_admin: bool,
}

/// TG-905: whether the caller is a system administrator — a plain answer for the main menu,
/// instead of probing `/api/admin/overview` and logging a 403 for every ordinary account.
#[utoipa::path(
    get,
    path = "/api/admin/access",
    responses(
        (status = 200, description = "Whether the caller administers the system", body = AdminAccess),
        (status = 401, description = "Missing or expired session")
    )
)]
pub async fn access(
    axum::extract::State(state): axum::extract::State<crate::state::SharedState>,
    headers: HeaderMap,
) -> Result<axum::Json<AdminAccess>, StatusCode> {
    match require_admin(&state, &headers).await {
        Ok(_) => Ok(axum::Json(AdminAccess { is_admin: true })),
        Err(StatusCode::FORBIDDEN) => Ok(axum::Json(AdminAccess { is_admin: false })),
        Err(status) => Err(status),
    }
}
