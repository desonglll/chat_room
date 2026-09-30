//! HTTP translation for the video note interface in `super`. No domain rules live here.

use axum::{
    extract::{Multipart, Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use uuid::Uuid;

use super::model::{VideoNoteError, MAX_THUMBNAIL_BYTES, MAX_VIDEO_NOTE_BYTES};
use super::VideoNoteUpload;
use crate::models::{StoredMessage, User};
use crate::state::SharedState;
use crate::user_handlers::bearer_token;

async fn caller(state: &SharedState, headers: &HeaderMap) -> Result<User, VideoNoteError> {
    state
        .session_user(bearer_token(headers)?)
        .await?
        .ok_or(VideoNoteError::Unauthorized)
}

async fn read_field(
    field: &mut axum::extract::multipart::Field<'_>,
    limit: usize,
    too_large: VideoNoteError,
) -> Result<Vec<u8>, VideoNoteError> {
    let mut buffer = Vec::new();
    while let Some(chunk) = field
        .chunk()
        .await
        .map_err(|error| VideoNoteError::from(error.status()))?
    {
        if buffer.len() + chunk.len() > limit {
            return Err(too_large);
        }
        buffer.extend_from_slice(&chunk);
    }
    Ok(buffer)
}

/// Multipart fields: `file` (WebM or MP4 with a video track), `duration_ms` (the recorder's
/// measurement, used only when the container carries no duration), optional `thumbnail`
/// (JPEG, at most 16 KiB) and `reply_to`.
#[utoipa::path(
    post,
    path = "/api/chats/{id}/video_note",
    params(("id" = Uuid, Path, description = "Chat identifier")),
    responses(
        (status = 201, description = "The video note, with its `video_note` field", body = StoredMessage),
        (status = 400, description = "Malformed duration, thumbnail or field"),
        (status = 401, description = "Missing session"),
        (status = 403, description = "No send permission, or `voice_messages_restricted` (TG-505)"),
        (status = 404, description = "Chat not found or caller is not a member"),
        (status = 413, description = "File too large"),
        (status = 415, description = "Not a WebM/MP4 video")
    )
)]
pub async fn send(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(room_id): Path<Uuid>,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<StoredMessage>), VideoNoteError> {
    let user = caller(&state, &headers).await?;
    // Refuse before reading a byte of video when the caller may not send here at all.
    super::authorize_video_note_send(&state, room_id, &user).await?;
    let limit = state.max_upload_bytes().min(MAX_VIDEO_NOTE_BYTES);
    let mut bytes: Option<Vec<u8>> = None;
    let mut thumbnail = None;
    let mut client_duration_ms = None;
    let mut reply_to = None;
    let mut topic_id = None;
    while let Some(mut field) = multipart
        .next_field()
        .await
        .map_err(|error| VideoNoteError::from(error.status()))?
    {
        match field.name() {
            Some("file") if bytes.is_none() => {
                bytes = Some(read_field(&mut field, limit, VideoNoteError::TooLarge).await?);
            }
            Some("thumbnail") if thumbnail.is_none() => {
                let invalid = VideoNoteError::Invalid("invalid_thumbnail");
                let value = read_field(&mut field, MAX_THUMBNAIL_BYTES, invalid).await?;
                thumbnail = Some(value).filter(|value| !value.is_empty());
            }
            Some("duration_ms") => {
                client_duration_ms = Some(
                    text(field)
                        .await?
                        .trim()
                        .parse()
                        .map_err(|_| VideoNoteError::Invalid("invalid_duration"))?,
                );
            }
            Some("reply_to") => {
                let value = text(field).await?;
                if !value.trim().is_empty() {
                    reply_to = Some(
                        value
                            .trim()
                            .parse()
                            .map_err(|_| VideoNoteError::Invalid("invalid_reply_to"))?,
                    );
                }
            }
            Some("topic_id") => {
                let value = text(field).await?;
                if !value.trim().is_empty() {
                    topic_id = Some(
                        value
                            .trim()
                            .parse()
                            .map_err(|_| VideoNoteError::Invalid("invalid_topic"))?,
                    );
                }
            }
            _ => {}
        }
    }
    let bytes = bytes
        .filter(|bytes| !bytes.is_empty())
        .ok_or(VideoNoteError::Invalid("missing_file"))?;
    let upload = VideoNoteUpload {
        bytes,
        client_duration_ms,
        thumbnail,
        reply_to,
        topic_id,
    };
    let message = super::send_video_note(&state, room_id, &user, upload).await?;
    Ok((StatusCode::CREATED, Json(message)))
}

async fn text(field: axum::extract::multipart::Field<'_>) -> Result<String, VideoNoteError> {
    field
        .text()
        .await
        .map_err(|_| VideoNoteError::Invalid("bad_request"))
}

#[utoipa::path(
    post,
    path = "/api/messages/{message_id}/video_note/listened",
    params(("message_id" = Uuid, Path, description = "The video note")),
    responses(
        (status = 204, description = "Marked (idempotent; a no-op for the sender)"),
        (status = 401, description = "Missing session"),
        (status = 404, description = "No such video note, or not visible to the caller")
    )
)]
pub async fn listened(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(message_id): Path<Uuid>,
) -> Result<StatusCode, VideoNoteError> {
    let user = caller(&state, &headers).await?;
    super::mark_video_note_listened(&state, message_id, &user).await?;
    Ok(StatusCode::NO_CONTENT)
}
