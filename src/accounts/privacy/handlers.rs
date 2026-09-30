//! `GET /api/users/me/privacy` and `PUT /api/users/me/privacy/:key`.

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};

use super::store::PrivacyWriteError;
use super::{PrivacyKey, PrivacyRuleView, PrivacyRuleWrite, PrivacySettings};
use crate::models::User;
use crate::state::SharedState;
use crate::user_handlers::bearer_token;

async fn session_user(state: &SharedState, headers: &HeaderMap) -> Result<User, StatusCode> {
    let token = bearer_token(headers)?;
    state
        .session_user(token)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::UNAUTHORIZED)
}

#[utoipa::path(
    get,
    path = "/api/users/me/privacy",
    responses(
        (status = 200, description = "Every privacy dimension of the caller", body = PrivacySettings),
        (status = 401, description = "Missing or expired session")
    )
)]
pub async fn get_privacy(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> Result<Json<PrivacySettings>, StatusCode> {
    let user = session_user(&state, &headers).await?;
    state
        .privacy_settings(user.id)
        .await
        .map(Json)
        .map_err(|error| {
            tracing::error!("load privacy settings failed: {error}");
            StatusCode::INTERNAL_SERVER_ERROR
        })
}

#[utoipa::path(
    put,
    path = "/api/users/me/privacy/{key}",
    params(("key" = PrivacyKey, Path, description = "Privacy dimension")),
    request_body = PrivacyRuleWrite,
    responses(
        (status = 200, description = "The stored rule", body = PrivacyRuleView),
        (status = 400, description = "Unknown dimension, unknown account, or too many exceptions"),
        (status = 401, description = "Missing or expired session")
    )
)]
pub async fn put_privacy_rule(
    State(state): State<SharedState>,
    Path(key): Path<PrivacyKey>,
    headers: HeaderMap,
    Json(write): Json<PrivacyRuleWrite>,
) -> Result<Json<PrivacyRuleView>, StatusCode> {
    let user = session_user(&state, &headers).await?;
    match state.save_privacy_rule(user.id, key, &write).await {
        Ok(Ok(view)) => Ok(Json(view)),
        Ok(Err(PrivacyWriteError::TooManyExceptions | PrivacyWriteError::UnknownUser)) => {
            Err(StatusCode::BAD_REQUEST)
        }
        Err(error) => {
            tracing::error!("save privacy rule failed: {error}");
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}
