//! Authorization and input normalization shared by upload routes.

use axum::{http::HeaderMap, http::StatusCode, Json};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::state::SharedState;
use crate::user_handlers::bearer_token;

use super::upload_models::ChunkResponse;

const MAX_FILE_NAME_CHARS: usize = 255;

pub(super) async fn authorize(
    state: &SharedState,
    room_id: Uuid,
    headers: &HeaderMap,
) -> Result<(crate::models::Room, crate::models::User), StatusCode> {
    let room = state.room(room_id).await.ok_or(StatusCode::NOT_FOUND)?;
    if room.has_password {
        let password = headers
            .get("x-room-password")
            .and_then(|value| value.to_str().ok())
            .ok_or(StatusCode::UNAUTHORIZED)?;
        let mut hasher = Sha256::new();
        hasher.update(password.as_bytes());
        if hex::encode(hasher.finalize()) != room.password_hash {
            return Err(StatusCode::UNAUTHORIZED);
        }
    }
    let token = bearer_token(headers)?;
    let user = state
        .session_user(token)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::UNAUTHORIZED)?;
    if !state
        .has_room_permission(room_id, user.id, "message.send")
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    {
        return Err(StatusCode::FORBIDDEN);
    }
    Ok((room, user))
}

pub(super) fn normalize_file_name(value: &str) -> Result<String, StatusCode> {
    let name = value
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or("file")
        .trim()
        .chars()
        .filter(|character| !character.is_control())
        .collect::<String>();
    if name.is_empty() || name.chars().count() > MAX_FILE_NAME_CHARS {
        return Err(StatusCode::BAD_REQUEST);
    }
    Ok(name)
}

pub(super) fn normalize_content_hash(value: Option<&str>) -> Result<Option<String>, StatusCode> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(StatusCode::BAD_REQUEST);
    }
    Ok(Some(value.to_ascii_lowercase()))
}

pub(super) fn chunk_error(
    status: StatusCode,
    received_bytes: i64,
) -> (StatusCode, Json<ChunkResponse>) {
    (status, Json(ChunkResponse { received_bytes }))
}
