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
