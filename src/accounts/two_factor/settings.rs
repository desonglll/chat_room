//! Account-facing 2FA management under `/api/users/me/two-factor`.

use std::net::SocketAddr;

use axum::{
    extract::{ConnectInfo, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use uuid::Uuid;

use super::super::{
    auth_limits::require_auth_capacity,
    credentials::{bearer_token, hash_password, password_matches},
};
use super::{
    codes,
    store::{CodePurpose, CredentialRow},
    wire::{
        normalize_email, presented_password, valid_hint, well_formed_code, ChangeTwoFactorRequest,
        ConfirmRecoveryEmailRequest, DisableTwoFactorRequest, EnableTwoFactorRequest, MailedCode,
        RecoveryEmailRequest, TwoFactorStatus,
    },
};
use crate::{
    models::User,
    security::AuthAction,
    state::{AppState, SharedState},
};

fn internal(context: &'static str) -> impl Fn(sqlx::Error) -> StatusCode {
    move |error| {
        tracing::error!("{context}: {error}");
        StatusCode::INTERNAL_SERVER_ERROR
    }
}

async fn session(state: &AppState, headers: &HeaderMap) -> Result<(Uuid, User), StatusCode> {
    let token = bearer_token(headers)?;
    let user = state
        .session_user(token)
        .await
        .map_err(internal("load session for two-factor settings failed"))?
        .ok_or(StatusCode::UNAUTHORIZED)?;
    Ok((token, user))
}

/// Rate-limit, then prove the presented 2FA password. `404` when 2FA is off.
async fn prove_second_factor(
    state: &SharedState,
    headers: &HeaderMap,
    peer: Option<ConnectInfo<SocketAddr>>,
    user: &User,
    password: String,
) -> Result<CredentialRow, StatusCode> {
    presented_password(&password)?;
    let account = user.id.to_string();
    require_auth_capacity(state, headers, peer, AuthAction::TwoFactor, &account).await?;
    let credential = state
        .two_factor_credential(user.id)
        .await
        .map_err(internal("load two-factor credential failed"))?
        .ok_or(StatusCode::NOT_FOUND)?;
    if !password_matches(password, credential.password_hash.clone()).await {
        return Err(StatusCode::UNAUTHORIZED);
    }
    Ok(credential)
}

async fn current_status(state: &AppState, user_id: Uuid) -> Result<TwoFactorStatus, StatusCode> {
    let Some(credential) = state
        .two_factor_credential(user_id)
        .await
        .map_err(internal("load two-factor credential failed"))?
    else {
        return Ok(TwoFactorStatus::default());
    };
    let pending = state
        .pending_code_email(user_id, CodePurpose::VerifyEmail)
        .await
        .map_err(internal("load pending recovery email failed"))?;
    Ok(TwoFactorStatus {
        enabled: true,
        hint: credential.hint,
        recovery_email: credential.recovery_email,
        pending_recovery_email: pending,
    })
}

#[utoipa::path(
    get,
    path = "/api/users/me/two-factor",
    responses(
        (status = 200, description = "2FA state of the current account", body = TwoFactorStatus),
        (status = 401, description = "Missing or expired session")
    )
)]
pub(crate) async fn status(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> Result<Json<TwoFactorStatus>, StatusCode> {
    let (_, user) = session(&state, &headers).await?;
    current_status(&state, user.id).await.map(Json)
}

/// Turn 2FA on. Every other device session of the account ends (D-010).
#[utoipa::path(
    post,
    path = "/api/users/me/two-factor",
    request_body = EnableTwoFactorRequest,
    responses(
        (status = 200, description = "2FA enabled; other sessions terminated", body = TwoFactorStatus),
        (status = 400, description = "Invalid password or hint"),
        (status = 401, description = "Incorrect account password or expired session"),
        (status = 409, description = "2FA is already enabled"),
        (status = 429, description = "Too many attempts")
    )
)]
pub(crate) async fn enable(
    State(state): State<SharedState>,
    peer: Option<ConnectInfo<SocketAddr>>,
    headers: HeaderMap,
    Json(request): Json<EnableTwoFactorRequest>,
) -> Result<Json<TwoFactorStatus>, StatusCode> {
    let (token, user) = session(&state, &headers).await?;
    presented_password(&request.password)?;
    presented_password(&request.account_password)?;
    let hint = valid_hint(&request.hint, &request.password)?;
    let account = user.id.to_string();
    require_auth_capacity(&state, &headers, peer, AuthAction::TwoFactor, &account).await?;
    let (_, account_hash) = state
        .user_credentials(&user.username)
        .await
        .map_err(internal("load account password failed"))?
        .ok_or(StatusCode::UNAUTHORIZED)?;
    if !password_matches(request.account_password, account_hash).await {
        return Err(StatusCode::UNAUTHORIZED);
    }
    let password_hash = hash_password(request.password).await?;
    if !state
        .enable_two_factor(user.id, &password_hash, &hint, token)
        .await
        .map_err(internal("enable two-factor failed"))?
    {
        return Err(StatusCode::CONFLICT);
    }
    current_status(&state, user.id).await.map(Json)
}

/// Change the 2FA password and/or hint. Sessions are kept.
#[utoipa::path(
    put,
    path = "/api/users/me/two-factor",
    request_body = ChangeTwoFactorRequest,
    responses(
        (status = 200, description = "2FA updated", body = TwoFactorStatus),
        (status = 400, description = "Invalid password or hint"),
        (status = 401, description = "Incorrect 2FA password or expired session"),
        (status = 404, description = "2FA is not enabled"),
        (status = 429, description = "Too many attempts")
    )
)]
pub(crate) async fn change(
    State(state): State<SharedState>,
    peer: Option<ConnectInfo<SocketAddr>>,
    headers: HeaderMap,
    Json(request): Json<ChangeTwoFactorRequest>,
) -> Result<Json<TwoFactorStatus>, StatusCode> {
    let (_, user) = session(&state, &headers).await?;
    if let Some(new_password) = &request.new_password {
        presented_password(new_password)?;
    }
    let effective = request
        .new_password
        .clone()
        .unwrap_or_else(|| request.current_password.clone());
    let credential =
        prove_second_factor(&state, &headers, peer, &user, request.current_password).await?;
    let hint = valid_hint(
        request.hint.as_deref().unwrap_or(&credential.hint),
        &effective,
    )?;
    let password_hash = match request.new_password {
        Some(password) => Some(hash_password(password).await?),
        None => None,
    };
    state
        .update_two_factor(user.id, password_hash.as_deref(), &hint)
        .await
        .map_err(internal("update two-factor failed"))?;
    current_status(&state, user.id).await.map(Json)
}

#[utoipa::path(
    delete,
    path = "/api/users/me/two-factor",
    request_body = DisableTwoFactorRequest,
    responses(
        (status = 204, description = "2FA disabled"),
        (status = 401, description = "Incorrect 2FA password or expired session"),
        (status = 404, description = "2FA is not enabled"),
        (status = 429, description = "Too many attempts")
    )
)]
pub(crate) async fn disable(
    State(state): State<SharedState>,
    peer: Option<ConnectInfo<SocketAddr>>,
    headers: HeaderMap,
    Json(request): Json<DisableTwoFactorRequest>,
) -> Result<StatusCode, StatusCode> {
    let (_, user) = session(&state, &headers).await?;
    prove_second_factor(&state, &headers, peer, &user, request.current_password).await?;
    state
        .remove_two_factor(user.id, None)
        .await
        .map_err(internal("disable two-factor failed"))?;
    Ok(StatusCode::NO_CONTENT)
}

/// Start (or restart) verification of a recovery address.
#[utoipa::path(
    post,
    path = "/api/users/me/two-factor/recovery-email",
    request_body = RecoveryEmailRequest,
    responses(
        (status = 202, description = "Verification code mailed", body = MailedCode),
        (status = 400, description = "Invalid email"),
        (status = 401, description = "Incorrect 2FA password or expired session"),
        (status = 404, description = "2FA is not enabled"),
        (status = 429, description = "Too many attempts"),
        (status = 503, description = "This server has no mail transport configured")
    )
)]
pub(crate) async fn request_recovery_email(
    State(state): State<SharedState>,
    peer: Option<ConnectInfo<SocketAddr>>,
    headers: HeaderMap,
    Json(request): Json<RecoveryEmailRequest>,
) -> Result<(StatusCode, Json<MailedCode>), StatusCode> {
    let (_, user) = session(&state, &headers).await?;
    let email = normalize_email(&request.email)?;
    prove_second_factor(&state, &headers, peer, &user, request.current_password).await?;
    let mailed = codes::issue(&state, user.id, CodePurpose::VerifyEmail, &email).await?;
    Ok((StatusCode::ACCEPTED, Json(mailed)))
}

#[utoipa::path(
    post,
    path = "/api/users/me/two-factor/recovery-email/confirm",
    request_body = ConfirmRecoveryEmailRequest,
    responses(
        (status = 200, description = "Recovery email verified", body = TwoFactorStatus),
        (status = 401, description = "Incorrect code or expired session"),
        (status = 410, description = "No live verification code"),
        (status = 429, description = "Too many attempts")
    )
)]
pub(crate) async fn confirm_recovery_email(
    State(state): State<SharedState>,
    peer: Option<ConnectInfo<SocketAddr>>,
    headers: HeaderMap,
    Json(request): Json<ConfirmRecoveryEmailRequest>,
) -> Result<Json<TwoFactorStatus>, StatusCode> {
    let (_, user) = session(&state, &headers).await?;
    let code = well_formed_code(&request.code)?;
    let account = user.id.to_string();
    require_auth_capacity(
        &state,
        &headers,
        peer,
        AuthAction::TwoFactorRecovery,
        &account,
    )
    .await?;
    let email = codes::verify(&state, user.id, CodePurpose::VerifyEmail, code).await?;
    if !state
        .set_recovery_email(user.id, &email)
        .await
        .map_err(internal("store recovery email failed"))?
    {
        return Err(StatusCode::NOT_FOUND);
    }
    current_status(&state, user.id).await.map(Json)
}
