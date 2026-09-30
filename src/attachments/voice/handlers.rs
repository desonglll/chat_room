//! HTTP translation for the voice interface in `super`. No domain rules live here.

use axum::{
    extract::{Multipart, Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use uuid::Uuid;

use super::model::{parse_waveform, VoiceError, MAX_VOICE_BYTES};
use super::VoiceUpload;
use crate::models::{StoredMessage, User};
use crate::state::SharedState;
use crate::user_handlers::bearer_token;

async fn caller(state: &SharedState, headers: &HeaderMap) -> Result<User, VoiceError> {
    state
        .session_user(bearer_token(headers)?)
        .await?
        .ok_or(VoiceError::Unauthorized)
}

/// Multipart fields: `file` (Ogg/Opus, WebM/Opus or MP4/AAC), `waveform` (100 comma-separated
/// integers 0-31), `duration_ms` (the recorder's measurement, used only when the container
/// carries no duration), optional `reply_to`.
#[utoipa::path(
    post,
    path = "/api/chats/{id}/voice",
    params(("id" = Uuid, Path, description = "Chat identifier")),
    responses(
        (status = 201, description = "The voice message, with its `voice` field", body = StoredMessage),
        (status = 400, description = "Malformed waveform, duration or field"),
        (status = 401, description = "Missing session"),
        (status = 403, description = "No send permission, or `voice_messages_restricted` (TG-505)"),
        (status = 404, description = "Chat not found or caller is not a member"),
        (status = 413, description = "File too large"),
        (status = 415, description = "Not a supported audio container")
    )
)]
pub async fn send(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(room_id): Path<Uuid>,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<StoredMessage>), VoiceError> {
    let user = caller(&state, &headers).await?;
    // Refuse before reading a byte of audio when the caller may not send here at all.
    super::authorize_voice_send(&state, room_id, &user).await?;
    let limit = state.max_upload_bytes().min(MAX_VOICE_BYTES);
    let mut bytes: Option<Vec<u8>> = None;
    let mut waveform = None;
    let mut client_duration_ms = None;
    let mut reply_to = None;
    let mut topic_id = None;
    while let Some(mut field) = multipart
        .next_field()
        .await
        .map_err(|error| VoiceError::from(error.status()))?
    {
        match field.name() {
            Some("file") if bytes.is_none() => {
                let mut buffer = Vec::new();
                while let Some(chunk) = field
                    .chunk()
                    .await
                    .map_err(|error| VoiceError::from(error.status()))?
                {
                    if buffer.len() + chunk.len() > limit {
                        return Err(VoiceError::TooLarge);
                    }
                    buffer.extend_from_slice(&chunk);
                }
                bytes = Some(buffer);
            }
            Some("waveform") => waveform = Some(parse_waveform(&text(field).await?)?),
            Some("duration_ms") => {
                client_duration_ms = Some(
                    text(field)
                        .await?
                        .trim()
                        .parse()
                        .map_err(|_| VoiceError::Invalid("invalid_duration"))?,
                );
            }
            Some("reply_to") => {
                let value = text(field).await?;
                if !value.trim().is_empty() {
                    reply_to = Some(
                        value
                            .trim()
                            .parse()
                            .map_err(|_| VoiceError::Invalid("invalid_reply_to"))?,
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
                            .map_err(|_| VoiceError::Invalid("invalid_topic"))?,
                    );
                }
            }
            _ => {}
        }
    }
    let bytes = bytes
        .filter(|bytes| !bytes.is_empty())
        .ok_or(VoiceError::Invalid("missing_file"))?;
    let upload = VoiceUpload {
        bytes,
        client_duration_ms,
        waveform: waveform.ok_or(VoiceError::Invalid("invalid_waveform"))?,
        reply_to,
        topic_id,
    };
    let message = super::send_voice(&state, room_id, &user, upload).await?;
    Ok((StatusCode::CREATED, Json(message)))
}

async fn text(field: axum::extract::multipart::Field<'_>) -> Result<String, VoiceError> {
    field
        .text()
        .await
        .map_err(|_| VoiceError::Invalid("bad_request"))
}

#[utoipa::path(
    post,
    path = "/api/messages/{message_id}/voice/listened",
    params(("message_id" = Uuid, Path, description = "The voice message")),
    responses(
        (status = 204, description = "Marked (idempotent; a no-op for the sender)"),
        (status = 401, description = "Missing session"),
        (status = 404, description = "No such voice message, or not visible to the caller")
    )
)]
pub async fn listened(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(message_id): Path<Uuid>,
) -> Result<StatusCode, VoiceError> {
    let user = caller(&state, &headers).await?;
    super::mark_voice_listened(&state, message_id, &user).await?;
    Ok(StatusCode::NO_CONTENT)
}
