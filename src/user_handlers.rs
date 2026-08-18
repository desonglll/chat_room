//! Registration, login, and session endpoints.

use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use axum::{
    extract::State,
    http::{header::AUTHORIZATION, HeaderMap, StatusCode},
    Json,
};
use uuid::Uuid;

use crate::models::{
    AuthRequest, AuthSession, ChangePasswordRequest, DeleteAccountRequest, UpdateProfileRequest,
    User,
};
use crate::state::SharedState;

const MAX_USERNAME_CHARS: usize = 48;
const MIN_PASSWORD_CHARS: usize = 8;
const MAX_PASSWORD_CHARS: usize = 256;
const MAX_DISPLAY_NAME_CHARS: usize = 48;
const MAX_SIGNATURE_CHARS: usize = 160;
const MAX_HOMEPAGE_CHARS: usize = 240;

fn normalize_credentials(request: AuthRequest) -> Result<(String, String), StatusCode> {
    let username = request.username.trim().to_string();
    let password_chars = request.password.chars().count();
    if username.is_empty()
        || username.chars().count() > MAX_USERNAME_CHARS
        || !(MIN_PASSWORD_CHARS..=MAX_PASSWORD_CHARS).contains(&password_chars)
    {
        return Err(StatusCode::BAD_REQUEST);
    }
    Ok((username, request.password))
}

async fn hash_password(password: String) -> Result<String, StatusCode> {
    tokio::task::spawn_blocking(move || {
        let salt = SaltString::encode_b64(Uuid::new_v4().as_bytes())
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        Argon2::default()
            .hash_password(password.as_bytes(), &salt)
            .map(|hash| hash.to_string())
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
    })
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
}

async fn password_matches(password: String, encoded_hash: String) -> bool {
    tokio::task::spawn_blocking(move || {
        PasswordHash::new(&encoded_hash).is_ok_and(|hash| {
            Argon2::default()
                .verify_password(password.as_bytes(), &hash)
                .is_ok()
        })
    })
    .await
    .unwrap_or(false)
}

pub(crate) fn bearer_token(headers: &HeaderMap) -> Result<Uuid, StatusCode> {
    headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .and_then(|value| value.parse().ok())
        .ok_or(StatusCode::UNAUTHORIZED)
}

pub(crate) fn optional_bearer_token(headers: &HeaderMap) -> Option<Uuid> {
    headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .and_then(|value| value.parse().ok())
}

/// Register an account and immediately issue a login session.
#[utoipa::path(
    post,
    path = "/api/users/register",
    request_body = AuthRequest,
    responses(
        (status = 201, description = "Account registered", body = AuthSession),
        (status = 400, description = "Invalid username or password"),
        (status = 409, description = "Username already exists")
    )
)]
pub async fn register(
    State(state): State<SharedState>,
    Json(request): Json<AuthRequest>,
) -> Result<(StatusCode, Json<AuthSession>), StatusCode> {
    let (username, password) = normalize_credentials(request)?;
    let password_hash = hash_password(password).await?;
    let user = match state.insert_user(&username, &password_hash).await {
        Ok(user) => user,
        Err(sqlx::Error::Database(error)) if error.is_unique_violation() => {
            return Err(StatusCode::CONFLICT)
        }
        Err(error) => {
            tracing::error!("register user failed: {}", error);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };
    state
        .create_session(user)
        .await
        .map(|session| (StatusCode::CREATED, Json(session)))
        .map_err(|error| {
            tracing::error!("create registration session failed: {}", error);
            StatusCode::INTERNAL_SERVER_ERROR
        })
}

/// Authenticate an account and issue a new login session.
#[utoipa::path(
    post,
    path = "/api/users/login",
    request_body = AuthRequest,
    responses(
        (status = 200, description = "Login succeeded", body = AuthSession),
        (status = 400, description = "Invalid username or password format"),
        (status = 401, description = "Incorrect username or password")
    )
)]
pub async fn login(
    State(state): State<SharedState>,
    Json(request): Json<AuthRequest>,
) -> Result<Json<AuthSession>, StatusCode> {
    let (username, password) = normalize_credentials(request)?;
    let Some((user, password_hash)) = state.user_credentials(&username).await.map_err(|error| {
        tracing::error!("load login credentials failed: {}", error);
        StatusCode::INTERNAL_SERVER_ERROR
    })?
    else {
        return Err(StatusCode::UNAUTHORIZED);
    };
    if !password_matches(password, password_hash).await {
        return Err(StatusCode::UNAUTHORIZED);
    }
    state.create_session(user).await.map(Json).map_err(|error| {
        tracing::error!("create login session failed: {}", error);
        StatusCode::INTERNAL_SERVER_ERROR
    })
}

/// Return the account represented by the bearer session token.
#[utoipa::path(
    get,
    path = "/api/users/me",
    responses(
        (status = 200, description = "Current account", body = User),
        (status = 401, description = "Missing or expired session")
    )
)]
pub async fn me(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> Result<Json<User>, StatusCode> {
    let token = bearer_token(&headers)?;
    state
        .session_user(token)
        .await
        .map_err(|error| {
            tracing::error!("load current session failed: {}", error);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .map(Json)
        .ok_or(StatusCode::UNAUTHORIZED)
}

/// Update editable fields on the current account.
#[utoipa::path(
    patch,
    path = "/api/users/me",
    request_body = UpdateProfileRequest,
    responses(
        (status = 200, description = "Profile updated", body = User),
        (status = 400, description = "Invalid profile value"),
        (status = 401, description = "Missing or expired session")
    )
)]
pub async fn update_me(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(request): Json<UpdateProfileRequest>,
) -> Result<Json<User>, StatusCode> {
    let token = bearer_token(&headers)?;
    let current = state
        .session_user(token)
        .await
        .map_err(|error| {
            tracing::error!("load current session for profile update failed: {}", error);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or(StatusCode::UNAUTHORIZED)?;
    let avatar_emoji = request
        .avatar_emoji
        .as_deref()
        .unwrap_or(&current.avatar_emoji)
        .trim();
    if avatar_emoji.chars().count() > 8 || avatar_emoji.chars().any(char::is_control) {
        return Err(StatusCode::BAD_REQUEST);
    }
    let display_name = request.display_name.as_deref().unwrap_or(&current.display_name).trim();
    let signature = request.signature.as_deref().unwrap_or(&current.signature).trim();
    let homepage = request.homepage.as_deref().unwrap_or(&current.homepage).trim();
    let valid_homepage = homepage.is_empty()
        || homepage.starts_with("https://")
        || homepage.starts_with("http://");
    if display_name.chars().count() > MAX_DISPLAY_NAME_CHARS
        || signature.chars().count() > MAX_SIGNATURE_CHARS
        || signature.chars().any(char::is_control)
        || homepage.chars().count() > MAX_HOMEPAGE_CHARS
        || homepage.chars().any(char::is_control)
        || !valid_homepage
    {
        return Err(StatusCode::BAD_REQUEST);
    }
    let updated = state
        .update_user_profile(current.id, avatar_emoji, display_name, signature, homepage)
        .await
        .map_err(|error| {
            tracing::error!("update user avatar failed: {}", error);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or(StatusCode::UNAUTHORIZED)?;
    state.publish_member_profile(&updated).await;
    Ok(Json(updated))
}

pub async fn change_password(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(request): Json<ChangePasswordRequest>,
) -> Result<StatusCode, StatusCode> {
    let token = bearer_token(&headers)?;
    let current = state
        .session_user(token)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::UNAUTHORIZED)?;
    let new_length = request.new_password.chars().count();
    if !(MIN_PASSWORD_CHARS..=MAX_PASSWORD_CHARS).contains(&new_length) {
        return Err(StatusCode::BAD_REQUEST);
    }
    let Some((_, password_hash)) = state
        .user_credentials(&current.username)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    else {
        return Err(StatusCode::UNAUTHORIZED);
    };
    if !password_matches(request.current_password, password_hash).await {
        return Err(StatusCode::UNAUTHORIZED);
    }
    let password_hash = hash_password(request.new_password).await?;
    state
        .update_user_password(current.id, &password_hash, token)
        .await
        .map_err(|error| {
            tracing::error!("change account password failed: {error}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn delete_account(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(request): Json<DeleteAccountRequest>,
) -> Result<StatusCode, StatusCode> {
    let token = bearer_token(&headers)?;
    let current = state
        .session_user(token)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::UNAUTHORIZED)?;
    let Some((_, password_hash)) = state
        .user_credentials(&current.username)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    else {
        return Err(StatusCode::UNAUTHORIZED);
    };
    if !password_matches(request.current_password, password_hash).await {
        return Err(StatusCode::UNAUTHORIZED);
    }

    for room in state.list_rooms(None).await {
        if room.creator_user_id == Some(current.id) {
            state
                .delete_room(room.id, &room.password_hash)
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        }
    }
    state.delete_user(current.id).await.map_err(|error| {
        tracing::error!("delete account failed: {error}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    Ok(StatusCode::NO_CONTENT)
}

/// Revoke the current bearer session.
#[utoipa::path(
    post,
    path = "/api/users/logout",
    responses(
        (status = 204, description = "Session revoked"),
        (status = 401, description = "Missing or expired session")
    )
)]
pub async fn logout(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> Result<StatusCode, StatusCode> {
    let token = bearer_token(&headers)?;
    state
        .delete_session(token)
        .await
        .map_err(|error| {
            tracing::error!("delete session failed: {}", error);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .then_some(StatusCode::NO_CONTENT)
        .ok_or(StatusCode::UNAUTHORIZED)
}
