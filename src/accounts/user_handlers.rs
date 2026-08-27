//! Registration, login, and session endpoints.

use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use axum::{
    extract::{ConnectInfo, Path, State},
    http::{header::AUTHORIZATION, HeaderMap, StatusCode},
    Json,
};
use std::net::SocketAddr;
use uuid::Uuid;

use crate::models::{
    AuthRequest, AuthSession, ChangePasswordRequest, DeleteAccountRequest, UpdateProfileRequest,
    User, VerifyPasswordRequest,
};
use crate::security::AuthAction;
use crate::state::SharedState;

use super::auth_limits::require_auth_capacity;

const MAX_USERNAME_CHARS: usize = 48;
const MIN_PASSWORD_CHARS: usize = 8;
const MAX_PASSWORD_CHARS: usize = 256;
const MAX_DISPLAY_NAME_CHARS: usize = 48;
const MAX_SIGNATURE_CHARS: usize = 160;
const MAX_HOMEPAGE_CHARS: usize = 240;

pub(super) fn normalize_credentials(
    username: String,
    password: String,
) -> Result<(String, String), StatusCode> {
    let username = username.trim().to_string();
    let password_chars = password.chars().count();
    if username.is_empty()
        || username.chars().count() > MAX_USERNAME_CHARS
        || !(MIN_PASSWORD_CHARS..=MAX_PASSWORD_CHARS).contains(&password_chars)
    {
        return Err(StatusCode::BAD_REQUEST);
    }
    Ok((username, password))
}

pub(super) async fn hash_password(password: String) -> Result<String, StatusCode> {
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

/// Authenticate an account and issue a new login session.
#[utoipa::path(
    post,
    path = "/api/users/login",
    request_body = AuthRequest,
    responses(
        (status = 200, description = "Login succeeded", body = AuthSession),
        (status = 400, description = "Invalid username or password format"),
        (status = 401, description = "Incorrect username or password"),
        (status = 429, description = "Too many login attempts")
    )
)]
pub async fn login(
    State(state): State<SharedState>,
    peer: Option<ConnectInfo<SocketAddr>>,
    headers: HeaderMap,
    Json(request): Json<AuthRequest>,
) -> Result<Json<AuthSession>, StatusCode> {
    let (username, password) = normalize_credentials(request.username, request.password)?;
    require_auth_capacity(&state, &headers, peer, AuthAction::Login, &username).await?;
    let Some((user, password_hash)) = state.user_credentials(&username).await.map_err(|error| {
        tracing::error!("load login credentials failed: {}", error);
        StatusCode::INTERNAL_SERVER_ERROR
    })?
    else {
        hash_password(password).await?;
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

/// Return another account's public profile (for the message/member "view profile" card).
#[utoipa::path(
    get,
    path = "/api/users/{id}",
    responses(
        (status = 200, description = "Public profile", body = User),
        (status = 401, description = "Missing or expired session"),
        (status = 404, description = "User not found")
    )
)]
pub async fn get_user(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(user_id): Path<uuid::Uuid>,
) -> Result<Json<User>, StatusCode> {
    bearer_token(&headers)?;
    state
        .user_by_id(user_id)
        .await
        .map_err(|error| {
            tracing::error!("load user profile failed: {}", error);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
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
    let replaces_image = request
        .avatar_emoji
        .as_ref()
        .is_some_and(|avatar| !avatar.trim().starts_with("/api/users/"));
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
    let valid_avatar = avatar_emoji == current.avatar_emoji
        || (avatar_emoji.chars().count() <= 8 && !avatar_emoji.chars().any(char::is_control));
    if !valid_avatar {
        return Err(StatusCode::BAD_REQUEST);
    }
    let display_name = request
        .display_name
        .as_deref()
        .unwrap_or(&current.display_name)
        .trim();
    let signature = request
        .signature
        .as_deref()
        .unwrap_or(&current.signature)
        .trim();
    let homepage = request
        .homepage
        .as_deref()
        .unwrap_or(&current.homepage)
        .trim();
    let valid_homepage =
        homepage.is_empty() || homepage.starts_with("https://") || homepage.starts_with("http://");
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
    if replaces_image {
        if let Some(storage_key) =
            state
                .delete_user_avatar_file(current.id)
                .await
                .map_err(|error| {
                    tracing::error!("remove avatar metadata failed: {error}");
                    StatusCode::INTERNAL_SERVER_ERROR
                })?
        {
            if let Err(error) = state.attachment_store().remove(&storage_key).await {
                tracing::warn!("remove avatar file failed: {error:#}");
            }
        }
    }
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

/// Verify the current account password without issuing another login session.
#[utoipa::path(
    post,
    path = "/api/users/me/verify-password",
    request_body = VerifyPasswordRequest,
    responses(
        (status = 204, description = "Password verified"),
        (status = 400, description = "Invalid password format"),
        (status = 401, description = "Incorrect password or expired session"),
        (status = 429, description = "Too many password verification attempts")
    )
)]
pub async fn verify_password(
    State(state): State<SharedState>,
    peer: Option<ConnectInfo<SocketAddr>>,
    headers: HeaderMap,
    Json(request): Json<VerifyPasswordRequest>,
) -> Result<StatusCode, StatusCode> {
    if request.current_password.chars().count() > MAX_PASSWORD_CHARS {
        return Err(StatusCode::BAD_REQUEST);
    }
    let token = bearer_token(&headers)?;
    let current = state
        .session_user(token)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::UNAUTHORIZED)?;
    require_auth_capacity(
        &state,
        &headers,
        peer,
        AuthAction::VerifyPassword,
        &current.id.to_string(),
    )
    .await?;
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
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    delete,
    path = "/api/users/me",
    request_body = DeleteAccountRequest,
    responses(
        (status = 204, description = "Account deleted"),
        (status = 401, description = "Incorrect password or expired session"),
        (status = 409, description = "System administrators must have their role revoked first")
    )
)]
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
    if state.is_system_admin(current.id).await.map_err(|error| {
        tracing::error!("check administrator before account deletion failed: {error}");
        StatusCode::INTERNAL_SERVER_ERROR
    })? {
        return Err(StatusCode::CONFLICT);
    }

    for room in state.list_rooms(None).await {
        if room.creator_user_id == Some(current.id) {
            state
                .delete_room(room.id, &room.password_hash)
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        }
    }
    let avatar = state.avatar_file(current.id).await.map_err(|error| {
        tracing::error!("load avatar before account deletion failed: {error}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let favorite_attachment_ids =
        state
            .favorite_attachment_ids(current.id)
            .await
            .map_err(|error| {
                tracing::error!(
                    "load favorite attachments before account deletion failed: {error}"
                );
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
    let direct_room_ids = state.delete_user(current.id).await.map_err(|error| {
        tracing::error!("delete account failed: {error}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    if let Some(avatar) = avatar {
        if let Err(error) = state.attachment_store().remove(&avatar.storage_key).await {
            tracing::warn!("remove deleted account avatar failed: {error:#}");
        }
    }
    for attachment_id in favorite_attachment_ids {
        if let Err(error) = state
            .recompute_attachment_orphan_status(attachment_id)
            .await
        {
            tracing::warn!(%attachment_id, "recompute deleted favorite attachment failed: {error}");
        }
    }
    for room_id in direct_room_ids {
        state
            .remove_cached_room(room_id, "direct conversation closed")
            .await;
    }
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
