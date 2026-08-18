//! Resumable/chunked attachment upload endpoints: create session, append
//! chunks by offset (the resume handshake), complete, list, and cancel.

use axum::{
    body::Bytes,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::attachment_upload_sessions::AttachmentUploadSession;
use crate::models::StoredMessage;
use crate::state::SharedState;
use crate::user_handlers::bearer_token;

const MAX_FILE_NAME_CHARS: usize = 255;
const MAX_MESSAGE_CHARS: usize = 4096;

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateUploadRequest {
    pub file_name: String,
    pub mime_type: String,
    pub size_bytes: i64,
    #[serde(default)]
    pub fingerprint: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CreateUploadResponse {
    pub upload_id: Uuid,
    pub received_bytes: i64,
    pub declared_size_bytes: i64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ChunkResponse {
    pub received_bytes: i64,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CompleteUploadRequest {
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub reply_to: Option<Uuid>,
    #[serde(default)]
    pub is_sensitive: bool,
}

#[derive(Deserialize)]
pub struct ChunkQuery {
    offset: i64,
}

/// Create a new chunked upload session, or resume an existing in-progress one
/// for the same file (matched by `fingerprint`).
#[utoipa::path(
    post,
    path = "/api/rooms/{id}/attachments/uploads",
    request_body = CreateUploadRequest,
    responses(
        (status = 200, description = "Session created or resumed", body = CreateUploadResponse),
        (status = 400, description = "Invalid file name or size"),
        (status = 401, description = "Invalid account or room credentials"),
        (status = 413, description = "Declared size exceeds the configured upload limit")
    )
)]
pub async fn create_upload(
    State(state): State<SharedState>,
    Path(room_id): Path<Uuid>,
    headers: HeaderMap,
    Json(request): Json<CreateUploadRequest>,
) -> Result<Json<CreateUploadResponse>, StatusCode> {
    let (_, user) = authorize(&state, room_id, &headers).await?;
    let file_name = normalize_file_name(&request.file_name)?;
    if request.size_bytes <= 0 {
        return Err(StatusCode::BAD_REQUEST);
    }
    if request.size_bytes as usize > state.max_upload_bytes() {
        return Err(StatusCode::PAYLOAD_TOO_LARGE);
    }
    let session = state
        .create_or_resume_attachment_upload(
            room_id,
            user.id,
            &file_name,
            &request.mime_type,
            request.size_bytes,
            request.fingerprint.trim(),
        )
        .await
        .map_err(|error| {
            tracing::error!("create attachment upload session failed: {}", error);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(Json(CreateUploadResponse {
        upload_id: session.id,
        received_bytes: session.received_bytes,
        declared_size_bytes: session.declared_size_bytes,
    }))
}

/// Append one chunk at `offset`. A 409 with the session's actual `received_bytes`
/// means the caller's offset is stale — the resume handshake: retry from there.
#[utoipa::path(
    put,
    path = "/api/attachments/uploads/{id}/chunks",
    params(("id" = Uuid, description = "Upload session id"), ("offset" = i64, Query, description = "Byte offset this chunk starts at")),
    responses(
        (status = 200, description = "Chunk accepted", body = ChunkResponse),
        (status = 409, description = "Offset mismatch — retry at the returned received_bytes", body = ChunkResponse),
        (status = 401, description = "Not this upload's owner"),
        (status = 404, description = "Unknown upload session")
    )
)]
pub async fn upload_chunk(
    State(state): State<SharedState>,
    Path(upload_id): Path<Uuid>,
    Query(query): Query<ChunkQuery>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<ChunkResponse>, StatusCode> {
    let token = bearer_token(&headers)?;
    let user = state
        .session_user(token)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::UNAUTHORIZED)?;
    let session = state
        .attachment_upload(upload_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    if session.uploader_id != user.id {
        return Err(StatusCode::UNAUTHORIZED);
    }
    if session.status != "in_progress" {
        return Err(StatusCode::CONFLICT);
    }
    let next_size = session.received_bytes.saturating_add(body.len() as i64);
    if next_size > session.declared_size_bytes || next_size as usize > state.max_upload_bytes() {
        return Err(StatusCode::PAYLOAD_TOO_LARGE);
    }

    match state
        .attachment_store()
        .append_chunk(upload_id, query.offset as u64, &body)
        .await
    {
        Ok(new_size) => {
            let new_size = new_size as i64;
            state
                .update_attachment_upload_progress(upload_id, new_size)
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            Ok(Json(ChunkResponse {
                received_bytes: new_size,
            }))
        }
        Err(error) => {
            // Most likely an offset mismatch — report the real on-disk size so the
            // client can resume from the correct position instead of restarting.
            let actual = state
                .attachment_store()
                .chunked_upload_size(upload_id)
                .await
                .unwrap_or(0) as i64;
            tracing::warn!("attachment chunk rejected: {error:#}");
            let _ = state.update_attachment_upload_progress(upload_id, actual).await;
            Err(StatusCode::CONFLICT)
        }
    }
}

/// Finish a chunked upload once every byte has arrived, turning it into a
/// normal chat message — the response shape matches the single-shot upload.
#[utoipa::path(
    post,
    path = "/api/attachments/uploads/{id}/complete",
    request_body = CompleteUploadRequest,
    responses(
        (status = 201, description = "Attachment message created", body = StoredMessage),
        (status = 400, description = "Upload is not fully received yet"),
        (status = 401, description = "Not this upload's owner"),
        (status = 404, description = "Unknown upload session")
    )
)]
pub async fn complete_upload(
    State(state): State<SharedState>,
    Path(upload_id): Path<Uuid>,
    headers: HeaderMap,
    Json(request): Json<CompleteUploadRequest>,
) -> Result<(StatusCode, Json<StoredMessage>), StatusCode> {
    let token = bearer_token(&headers)?;
    let user = state
        .session_user(token)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::UNAUTHORIZED)?;
    let session = state
        .attachment_upload(upload_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    if session.uploader_id != user.id {
        return Err(StatusCode::UNAUTHORIZED);
    }
    if session.status != "in_progress" || session.received_bytes != session.declared_size_bytes {
        return Err(StatusCode::BAD_REQUEST);
    }
    let content = request.content.trim();
    if content.chars().count() > MAX_MESSAGE_CHARS {
        return Err(StatusCode::BAD_REQUEST);
    }

    let display_name = state.resolve_display_name(session.room_id, &user).await;
    let result = state
        .store_chunked_attachment_message(
            upload_id,
            session.room_id,
            &user,
            &display_name,
            session.file_name.clone(),
            session.mime_type.clone(),
            request.is_sensitive,
            content,
            request.reply_to,
        )
        .await;
    match result {
        Ok(message) => {
            let _ = state.finish_attachment_upload(upload_id, "completed").await;
            Ok((StatusCode::CREATED, Json(message)))
        }
        Err(error) => {
            tracing::error!("complete chunked attachment upload failed: {}", error);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// List the caller's own in-progress uploads in a room (resume discovery).
#[utoipa::path(
    get,
    path = "/api/rooms/{id}/attachments/uploads",
    responses((status = 200, description = "In-progress uploads", body = Vec<AttachmentUploadSession>))
)]
pub async fn list_uploads(
    State(state): State<SharedState>,
    Path(room_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Vec<AttachmentUploadSession>>, StatusCode> {
    let token = bearer_token(&headers)?;
    let user = state
        .session_user(token)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::UNAUTHORIZED)?;
    state
        .list_attachment_uploads(room_id, user.id)
        .await
        .map(Json)
        .map_err(|error| {
            tracing::error!("list attachment uploads failed: {}", error);
            StatusCode::INTERNAL_SERVER_ERROR
        })
}

/// Cancel an in-progress upload and discard its staged bytes.
#[utoipa::path(
    delete,
    path = "/api/attachments/uploads/{id}",
    responses((status = 204, description = "Cancelled"), (status = 404, description = "Not found or not owned by caller"))
)]
pub async fn cancel_upload(
    State(state): State<SharedState>,
    Path(upload_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<StatusCode, StatusCode> {
    let token = bearer_token(&headers)?;
    let user = state
        .session_user(token)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::UNAUTHORIZED)?;
    let removed = state
        .delete_attachment_upload(upload_id, user.id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if !removed {
        return Err(StatusCode::NOT_FOUND);
    }
    let _ = state.attachment_store().discard_chunked(upload_id).await;
    Ok(StatusCode::NO_CONTENT)
}

async fn authorize(
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
        use sha2::{Digest, Sha256};
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

fn normalize_file_name(value: &str) -> Result<String, StatusCode> {
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
