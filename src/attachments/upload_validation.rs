//! Authorization and input normalization shared by upload routes.

use axum::{http::HeaderMap, http::StatusCode, Json};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::state::SharedState;
use crate::user_handlers::bearer_token;

use super::upload_models::ChunkResponse;

const MAX_FILE_NAME_CHARS: usize = 255;
/// A caption's ceiling, as for a text message (moved here from `upload_handlers`).
pub(super) const MAX_MESSAGE_CHARS: usize = 4096;

pub(super) async fn authorize(
    state: &SharedState,
    room_id: Uuid,
    headers: &HeaderMap,
) -> Result<(crate::models::Chat, crate::models::User), StatusCode> {
    let chat = state.chat(room_id).await.ok_or(StatusCode::NOT_FOUND)?;
    if chat.has_password {
        let password = headers
            .get("x-room-password")
            .and_then(|value| value.to_str().ok())
            .ok_or(StatusCode::UNAUTHORIZED)?;
        let mut hasher = Sha256::new();
        hasher.update(password.as_bytes());
        if hex::encode(hasher.finalize()) != chat.password_hash {
            return Err(StatusCode::UNAUTHORIZED);
        }
    }
    let token = bearer_token(headers)?;
    let user = state
        .session_user(token)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::UNAUTHORIZED)?;
    // TG-201: uploading is sending media.
    if !state
        .has_chat_permission(room_id, user.id, "message.send_media")
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    {
        return Err(StatusCode::FORBIDDEN);
    }
    Ok((chat, user))
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

/// A multipart text field holding a UUID (`reply_to`, TG-204's `topic_id`); malformed is 400.
pub(super) async fn multipart_uuid(
    field: axum::extract::multipart::Field<'_>,
) -> Result<Option<Uuid>, StatusCode> {
    let text = field.text().await.map_err(|_| StatusCode::BAD_REQUEST)?;
    text.parse().map(Some).map_err(|_| StatusCode::BAD_REQUEST)
}

/// TG-204: a completed upload's reply target and forum topic, with the topic rule applied.
pub(super) async fn placement(
    state: &SharedState,
    room_id: Uuid,
    user_id: Uuid,
    request: &super::upload_models::CompleteUploadRequest,
) -> Result<crate::message_store::MessagePlacement, StatusCode> {
    let (reply_to, topic_id) = (request.reply_to, request.topic_id);
    Ok(state
        .placement(room_id, user_id, reply_to, topic_id)
        .await?)
}
