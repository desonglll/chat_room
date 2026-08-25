//! Resumable/chunked attachment upload endpoints: create session, append
//! chunks by offset (the resume handshake), complete, list, and cancel.

use axum::{
    body::Bytes,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use uuid::Uuid;

use crate::attachment_upload_sessions::{AttachmentUploadSession, AttachmentUploadSpec};
use crate::models::StoredMessage;
use crate::realtime::protocol::stored_message_to_chat;
use crate::state::SharedState;
use crate::user_handlers::bearer_token;

use super::upload_validation::{
    authorize, chunk_error, normalize_content_hash, normalize_file_name,
};

const MAX_MESSAGE_CHARS: usize = 4096;
const MAX_DIRECT_UPLOAD_BYTES: i64 = 5 * 1024 * 1024 * 1024;

pub use super::upload_models::{
    ChunkResponse, CompleteUploadRequest, CreateUploadRequest, CreateUploadResponse,
};
#[derive(serde::Deserialize)]
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
        (status = 409, description = "Selected content does not match the resumable session"),
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
    let content_hash = normalize_content_hash(request.content_hash.as_deref())?;
    let reusable_storage_key = match content_hash.as_deref() {
        Some(hash) => state
            .healthy_owned_storage_key(hash, user.id, request.size_bytes)
            .await
            .map_err(|error| {
                tracing::error!("look up reusable attachment failed: {error}");
                StatusCode::INTERNAL_SERVER_ERROR
            })?,
        None => None,
    };
    let initial_received_bytes = if reusable_storage_key.is_some() {
        request.size_bytes
    } else {
        0
    };
    let session = state
        .create_or_resume_attachment_upload(AttachmentUploadSpec {
            room_id,
            uploader_id: user.id,
            file_name: &file_name,
            mime_type: &request.mime_type,
            declared_size_bytes: request.size_bytes,
            fingerprint: request.fingerprint.trim(),
            content_hash: content_hash.as_deref(),
            initial_received_bytes,
        })
        .await
        .map_err(|error| {
            tracing::error!("create attachment upload session failed: {}", error);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    if matches!(
        (session.content_hash.as_deref(), content_hash.as_deref()),
        (Some(existing), Some(requested)) if existing != requested
    ) {
        return Err(StatusCode::CONFLICT);
    }
    if reusable_storage_key.is_some() {
        state
            .attachment_store()
            .discard_chunked(session.id)
            .await
            .map_err(|error| {
                tracing::error!("discard superseded upload staging failed: {error:#}");
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
    }
    let direct_upload = if content_hash.is_some()
        && reusable_storage_key.is_none()
        && session.received_bytes == 0
        && session.declared_size_bytes <= MAX_DIRECT_UPLOAD_BYTES
    {
        match state
            .attachment_store()
            .presign_upload(session.id, &session.mime_type)
            .await
        {
            Ok(target) => target,
            Err(error) => {
                tracing::warn!(
                    upload_id = %session.id,
                    "direct OSS upload signing failed; using server upload: {error:#}"
                );
                None
            }
        }
    } else {
        None
    };
    Ok(Json(CreateUploadResponse {
        upload_id: session.id,
        received_bytes: session.received_bytes,
        declared_size_bytes: session.declared_size_bytes,
        deduplicated: reusable_storage_key.is_some(),
        direct_upload,
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
) -> Result<Json<ChunkResponse>, (StatusCode, Json<ChunkResponse>)> {
    let token = bearer_token(&headers).map_err(|status| chunk_error(status, 0))?;
    let user = state
        .session_user(token)
        .await
        .map_err(|_| chunk_error(StatusCode::INTERNAL_SERVER_ERROR, 0))?
        .ok_or_else(|| chunk_error(StatusCode::UNAUTHORIZED, 0))?;
    let session = state
        .attachment_upload(upload_id)
        .await
        .map_err(|_| chunk_error(StatusCode::INTERNAL_SERVER_ERROR, 0))?
        .ok_or_else(|| chunk_error(StatusCode::NOT_FOUND, 0))?;
    if session.uploader_id != user.id {
        return Err(chunk_error(
            StatusCode::UNAUTHORIZED,
            session.received_bytes,
        ));
    }
    if session.status != "in_progress" {
        return Err(chunk_error(StatusCode::CONFLICT, session.received_bytes));
    }
    let next_size = session.received_bytes.saturating_add(body.len() as i64);
    if next_size > session.declared_size_bytes || next_size as usize > state.max_upload_bytes() {
        return Err(chunk_error(
            StatusCode::PAYLOAD_TOO_LARGE,
            session.received_bytes,
        ));
    }
    let _permit = state
        .work_queue()
        .upload()
        .await
        .map_err(|_| chunk_error(StatusCode::SERVICE_UNAVAILABLE, session.received_bytes))?;

    match state
        .attachment_store()
        .append_chunk(upload_id, query.offset as u64, &body)
        .await
    {
        Ok(new_size) => {
            let new_size = new_size as i64;
            state
                .upload_hashes()
                .record(upload_id, query.offset as u64, &body)
                .await;
            state
                .update_attachment_upload_progress(upload_id, new_size)
                .await
                .map_err(|_| chunk_error(StatusCode::INTERNAL_SERVER_ERROR, new_size))?;
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
            let _ = state
                .update_attachment_upload_progress(upload_id, actual)
                .await;
            Err(chunk_error(StatusCode::CONFLICT, actual))
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
    let direct_upload = session.received_bytes == 0
        && session.content_hash.is_some()
        && session.declared_size_bytes <= MAX_DIRECT_UPLOAD_BYTES
        && state.attachment_store().direct_upload_enabled();
    if session.status != "in_progress"
        || (!direct_upload && session.received_bytes != session.declared_size_bytes)
    {
        return Err(StatusCode::BAD_REQUEST);
    }
    let content = request.content.trim();
    if content.chars().count() > MAX_MESSAGE_CHARS {
        return Err(StatusCode::BAD_REQUEST);
    }
    let _permit = state
        .work_queue()
        .upload()
        .await
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    let display_name = state.resolve_display_name(session.room_id, &user).await;
    let streamed_hash = if direct_upload {
        None
    } else {
        state
            .upload_hashes()
            .completed_digest(upload_id, session.declared_size_bytes as u64)
            .await
    };
    let reusable_storage_key = match session.content_hash.as_deref() {
        Some(hash) => state
            .healthy_owned_storage_key(hash, user.id, session.declared_size_bytes)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
        None => None,
    };
    let result = if direct_upload {
        state
            .store_direct_attachment_message(
                upload_id,
                session.room_id,
                &user,
                &display_name,
                session.file_name.clone(),
                session.mime_type.clone(),
                session.declared_size_bytes,
                request.is_sensitive,
                content,
                request.reply_to,
                session
                    .content_hash
                    .clone()
                    .expect("direct upload requires a content hash"),
            )
            .await
    } else if let (Some(content_hash), Some(storage_key)) =
        (session.content_hash.clone(), reusable_storage_key)
    {
        state
            .attachment_store()
            .discard_chunked(upload_id)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        state
            .store_existing_attachment_message(
                session.room_id,
                &user,
                &display_name,
                session.file_name.clone(),
                session.mime_type.clone(),
                session.declared_size_bytes,
                request.is_sensitive,
                content,
                request.reply_to,
                content_hash,
                storage_key,
            )
            .await
    } else {
        state
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
                session.content_hash.as_deref(),
                streamed_hash.as_deref(),
            )
            .await
    };
    match result {
        Ok(message) => {
            if !direct_upload {
                let _ = state.attachment_store().discard_direct(upload_id).await;
            }
            state.upload_hashes().remove(upload_id).await;
            let _ = state.finish_attachment_upload(upload_id, "completed").await;
            state.invalidate_message_cache(session.room_id).await;
            state
                .broadcast(session.room_id, stored_message_to_chat(message.clone()))
                .await;
            Ok((StatusCode::CREATED, Json(message)))
        }
        Err(error) => {
            tracing::error!("complete attachment upload failed: {error:#}");
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
    let _ = state.attachment_store().discard_direct(upload_id).await;
    state.upload_hashes().remove(upload_id).await;
    Ok(StatusCode::NO_CONTENT)
}
