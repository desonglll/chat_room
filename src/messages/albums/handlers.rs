//! HTTP translation for the album interface in `super`. No domain rules live here.

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::model::{AlbumError, RecallAlbumResponse, SendAlbumRequest, SendAlbumResponse};
use crate::models::User;
use crate::state::SharedState;
use crate::user_handlers::bearer_token;

async fn caller(state: &SharedState, headers: &HeaderMap) -> Result<User, AlbumError> {
    state
        .session_user(bearer_token(headers)?)
        .await?
        .ok_or(AlbumError::Unauthorized)
}

/// Password-protected chats ask for the password on every media write, like the upload
/// endpoints the album's files went through.
async fn check_chat_password(
    state: &SharedState,
    room_id: Uuid,
    headers: &HeaderMap,
) -> Result<(), AlbumError> {
    let chat = state.chat(room_id).await.ok_or(AlbumError::NotFound)?;
    if !chat.has_password {
        return Ok(());
    }
    let password = headers
        .get("x-room-password")
        .and_then(|value| value.to_str().ok())
        .ok_or(AlbumError::Unauthorized)?;
    if hex::encode(Sha256::digest(password.as_bytes())) != chat.password_hash {
        return Err(AlbumError::Unauthorized);
    }
    Ok(())
}

/// Send 2-10 completed photo/video uploads as one album, atomically.
#[utoipa::path(
    post,
    path = "/api/chats/{id}/albums",
    params(("id" = Uuid, Path, description = "Chat identifier")),
    request_body = SendAlbumRequest,
    responses(
        (status = 201, description = "Album created; every item was broadcast", body = SendAlbumResponse),
        (status = 400, description = "album_size, duplicate_upload, caption_too_long, upload_incomplete, not_media or reply_not_found"),
        (status = 401, description = "Missing session or wrong chat password"),
        (status = 403, description = "No permission to send media here"),
        (status = 404, description = "Chat or upload not found, or not the caller's"),
        (status = 409, description = "An upload was already turned into a message")
    )
)]
pub async fn send(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(room_id): Path<Uuid>,
    Json(request): Json<SendAlbumRequest>,
) -> Result<(StatusCode, Json<SendAlbumResponse>), AlbumError> {
    let user = caller(&state, &headers).await?;
    check_chat_password(&state, room_id, &headers).await?;
    let sent = super::send_album(&state, room_id, &user, request).await?;
    Ok((StatusCode::CREATED, Json(sent)))
}

/// Delete the whole album: every item the caller sent in it.
#[utoipa::path(
    delete,
    path = "/api/chats/{id}/albums/{grouped_id}",
    params(
        ("id" = Uuid, Path, description = "Chat identifier"),
        ("grouped_id" = Uuid, Path, description = "Album identifier")
    ),
    responses(
        (status = 200, description = "Recalled items; `message_recalled` was broadcast for each", body = RecallAlbumResponse),
        (status = 401, description = "Missing session"),
        (status = 404, description = "No visible item of this album was sent by the caller")
    )
)]
pub async fn delete(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path((room_id, grouped_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<RecallAlbumResponse>, AlbumError> {
    let user = caller(&state, &headers).await?;
    let recalled = super::recall_album(&state, room_id, &user, grouped_id).await?;
    Ok(Json(recalled))
}
