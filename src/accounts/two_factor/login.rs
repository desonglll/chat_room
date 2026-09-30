//! The second login stage and the forgot-2FA recovery path.

use std::net::SocketAddr;

use axum::{
    extract::{ConnectInfo, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use chrono::{Duration, Utc};
use uuid::Uuid;

use super::super::{
    auth_limits::require_auth_capacity, credentials::password_matches, sessions::SessionMetadata,
};
use super::{
    codes,
    store::CodePurpose,
    wire::{
        presented_password, well_formed_code, CompleteTwoFactorLoginRequest,
        ConfirmRecoveryRequest, MailedCode, RecoveryRequest, TwoFactorChallenge,
    },
};
use crate::{
    models::{AuthSession, User},
    security::AuthAction,
    state::{AppState, SharedState},
};

const CHALLENGE_LIFETIME_MINUTES: i64 = 5;
const MAX_CHALLENGE_ATTEMPTS: i32 = 5;

fn db_error(context: &'static str) -> impl Fn(sqlx::Error) -> StatusCode {
    move |error| {
        tracing::error!("{context}: {error}");
        StatusCode::INTERNAL_SERVER_ERROR
    }
}

/// Called by `POST /api/users/login` after the account password matched. `None` means the
/// account has no 2FA and the caller issues a session exactly as before TG-506.
pub(crate) async fn challenge_response(
    state: &AppState,
    user: &User,
) -> Result<Option<Response>, StatusCode> {
    let Some(credential) = state
        .two_factor_credential(user.id)
        .await
        .map_err(db_error("load two-factor credential at login failed"))?
    else {
        return Ok(None);
    };
    let expires_at = Utc::now() + Duration::minutes(CHALLENGE_LIFETIME_MINUTES);
    let pending_token = state
        .create_login_challenge(user.id, expires_at)
        .await
        .map_err(db_error("create two-factor login challenge failed"))?;
    let body = TwoFactorChallenge {
        error: "two_factor_required",
        pending_token,
        hint: credential.hint,
        has_recovery_email: credential.recovery_email.is_some(),
        expires_at,
    };
    Ok(Some(
        (StatusCode::PRECONDITION_REQUIRED, Json(body)).into_response(),
    ))
}

async fn issue_session(
    state: &AppState,
    user_id: Uuid,
    headers: &HeaderMap,
    peer: Option<ConnectInfo<SocketAddr>>,
) -> Result<Json<AuthSession>, StatusCode> {
    let user = state
        .user_by_id(user_id)
        .await
        .map_err(db_error("load two-factor login account failed"))?
        .ok_or(StatusCode::GONE)?;
    let metadata = SessionMetadata::from_request(headers, peer, state.trust_proxy_headers());
    state
        .create_session_with_metadata(user, metadata)
        .await
        .map(Json)
        .map_err(db_error("create two-factor login session failed"))
}

/// Exchange a pending token and the 2FA password for a session.
#[utoipa::path(
    post,
    path = "/api/users/login/two-factor",
    request_body = CompleteTwoFactorLoginRequest,
    responses(
        (status = 200, description = "Second factor accepted", body = AuthSession),
        (status = 401, description = "Incorrect 2FA password"),
        (status = 410, description = "Pending token unknown, expired, or out of attempts"),
        (status = 429, description = "Too many attempts")
    )
)]
pub(crate) async fn complete_login(
    State(state): State<SharedState>,
    peer: Option<ConnectInfo<SocketAddr>>,
    headers: HeaderMap,
    Json(request): Json<CompleteTwoFactorLoginRequest>,
) -> Result<Json<AuthSession>, StatusCode> {
    presented_password(&request.password)?;
    let user_id = state
        .login_challenge_user(request.pending_token)
        .await
        .map_err(db_error("load two-factor challenge failed"))?
        .ok_or(StatusCode::GONE)?;
    let account = user_id.to_string();
    require_auth_capacity(&state, &headers, peer, AuthAction::TwoFactor, &account).await?;
    state
        .reserve_login_attempt(request.pending_token, MAX_CHALLENGE_ATTEMPTS)
        .await
        .map_err(db_error("reserve two-factor attempt failed"))?
        .ok_or(StatusCode::GONE)?;
    let credential = state
        .two_factor_credential(user_id)
        .await
        .map_err(db_error("load two-factor credential failed"))?
        .ok_or(StatusCode::GONE)?;
    if !password_matches(request.password, credential.password_hash).await {
        return Err(StatusCode::UNAUTHORIZED);
    }
    state
        .consume_login_challenge(request.pending_token)
        .await
        .map_err(db_error("consume two-factor challenge failed"))?
        .filter(|consumed| *consumed == user_id)
        .ok_or(StatusCode::GONE)?;
    issue_session(&state, user_id, &headers, peer).await
}

/// Mail a reset code to the account's *verified* recovery address. Requires a pending
/// token, i.e. the account password was already proven.
#[utoipa::path(
    post,
    path = "/api/users/login/two-factor/recovery",
    request_body = RecoveryRequest,
    responses(
        (status = 202, description = "Reset code mailed", body = MailedCode),
        (status = 409, description = "No verified recovery email"),
        (status = 410, description = "Pending token unknown or expired"),
        (status = 429, description = "Too many attempts"),
        (status = 503, description = "This server has no mail transport configured")
    )
)]
pub(crate) async fn request_recovery(
    State(state): State<SharedState>,
    peer: Option<ConnectInfo<SocketAddr>>,
    headers: HeaderMap,
    Json(request): Json<RecoveryRequest>,
) -> Result<(StatusCode, Json<MailedCode>), StatusCode> {
    let user_id = state
        .login_challenge_user(request.pending_token)
        .await
        .map_err(db_error("load two-factor challenge failed"))?
        .ok_or(StatusCode::GONE)?;
    let account = user_id.to_string();
    require_auth_capacity(
        &state,
        &headers,
        peer,
        AuthAction::TwoFactorRecovery,
        &account,
    )
    .await?;
    let email = state
        .two_factor_credential(user_id)
        .await
        .map_err(db_error("load two-factor credential failed"))?
        .ok_or(StatusCode::GONE)?
        .recovery_email
        .ok_or(StatusCode::CONFLICT)?;
    let mailed = codes::issue(&state, user_id, CodePurpose::Reset, &email).await?;
    Ok((StatusCode::ACCEPTED, Json(mailed)))
}

/// Pending token + mailed reset code → 2FA removed, every other session ended, and a new
/// session for this device.
#[utoipa::path(
    post,
    path = "/api/users/login/two-factor/recovery/confirm",
    request_body = ConfirmRecoveryRequest,
    responses(
        (status = 200, description = "2FA reset; other sessions terminated", body = AuthSession),
        (status = 401, description = "Incorrect code"),
        (status = 410, description = "No live pending token or code"),
        (status = 429, description = "Too many attempts")
    )
)]
pub(crate) async fn confirm_recovery(
    State(state): State<SharedState>,
    peer: Option<ConnectInfo<SocketAddr>>,
    headers: HeaderMap,
    Json(request): Json<ConfirmRecoveryRequest>,
) -> Result<Json<AuthSession>, StatusCode> {
    let code = well_formed_code(&request.code)?;
    let user_id = state
        .login_challenge_user(request.pending_token)
        .await
        .map_err(db_error("load two-factor challenge failed"))?
        .ok_or(StatusCode::GONE)?;
    let account = user_id.to_string();
    require_auth_capacity(
        &state,
        &headers,
        peer,
        AuthAction::TwoFactorRecovery,
        &account,
    )
    .await?;
    codes::verify(&state, user_id, CodePurpose::Reset, code).await?;
    state
        .consume_login_challenge(request.pending_token)
        .await
        .map_err(db_error("consume two-factor challenge failed"))?
        .filter(|consumed| *consumed == user_id)
        .ok_or(StatusCode::GONE)?;
    // No session is kept: the new one below is created after every old one is gone.
    state
        .remove_two_factor(user_id, Some(Uuid::nil()))
        .await
        .map_err(db_error("reset two-factor after recovery failed"))?;
    issue_session(&state, user_id, &headers, peer).await
}
