//! Round video messages — "video notes" (TG-402).
//!
//! A video note is an attachment message with `media_kind = 'video_note'` plus a
//! `video_notes` row (duration, optional JPEG thumbnail), so it flows through the normal
//! history, realtime and download paths and only gains `StoredMessage::video_note`. It reuses
//! TG-401's pieces: the send authorization (including TG-505's "voice messages" rule, which
//! Telegram applies to video messages too), the container probe, the storage pipeline, and the
//! `voice_listened` frame for the watched state (one `video_note_listens` row per viewer).
//!
//! The functions below are the module interface; `handlers` only translates HTTP.
//! Contract: `docs/devlog/TG-402.md`, Frozen interface.

pub(crate) mod handlers;
pub mod model;
mod store;

use std::sync::Arc;

use axum::{extract::DefaultBodyLimit, routing::post, Router};
use uuid::Uuid;

use crate::attachments::voice::model::VoiceError;
use crate::attachments::voice::{authorize_voice_send, model::DurationSource, probe};
use crate::models::{ChatMessage, StoredMessage, User};
use crate::realtime::protocol::stored_message_to_chat;
use crate::state::{AppState, SharedState};
use model::{
    validate_thumbnail, VideoNoteError, MAX_THUMBNAIL_BYTES, MAX_VIDEO_NOTE_BYTES,
    MAX_VIDEO_NOTE_DURATION_MS,
};
use store::NewVideoNote;

/// The video note routes. Canonical prefix only (frozen clients know no video notes).
pub fn routes(multipart_body_limit: usize) -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/api/chats/:id/video_note",
            post(handlers::send).layer(DefaultBodyLimit::max(
                multipart_body_limit.min(MAX_VIDEO_NOTE_BYTES + MAX_THUMBNAIL_BYTES + 64 * 1024),
            )),
        )
        .route(
            "/api/messages/:message_id/video_note/listened",
            post(handlers::listened),
        )
}

/// Forwarding keeps a video note a video note (unwatched). Both statements run in the forward
/// transaction and are no-ops for any other source message.
/// Binds: `$1` new message id, `$2` source message id, `$3` created_at.
pub(crate) const FORWARD_COPY_VIDEO_NOTE: &str = "INSERT INTO video_notes \
    (message_id, duration_ms, thumbnail, duration_source, created_at) \
    SELECT $1, duration_ms, thumbnail, duration_source, $3 FROM video_notes WHERE message_id = $2";
/// Binds: `$1` new message id, `$2` source message id.
pub(crate) const FORWARD_MARK_VIDEO_NOTE: &str = "UPDATE messages SET media_kind = 'video_note' \
    WHERE id = $1 AND EXISTS (SELECT 1 FROM video_notes WHERE message_id = $2)";

/// A recorded video note as the handler received it.
pub struct VideoNoteUpload {
    pub bytes: Vec<u8>,
    /// The recorder's own measurement; used only when the container carries no duration.
    pub client_duration_ms: Option<u32>,
    /// Already validated by [`model::validate_thumbnail`].
    pub thumbnail: Option<Vec<u8>>,
    pub reply_to: Option<Uuid>,
    /// TG-204: the forum topic to post into; `None` = General.
    pub topic_id: Option<Uuid>,
}

/// Who may send a video note: exactly who may send a voice message (TG-401).
pub async fn authorize_video_note_send(
    state: &SharedState,
    room_id: Uuid,
    sender: &User,
) -> Result<(), VideoNoteError> {
    Ok(authorize_voice_send(state, room_id, sender).await?)
}

/// Validate, store and broadcast one video note. The container is sniffed from the bytes
/// (never trusted from the client) and must hold a video track; the duration is read from it
/// when it carries one.
pub async fn send_video_note(
    state: &SharedState,
    room_id: Uuid,
    sender: &User,
    upload: VideoNoteUpload,
) -> Result<StoredMessage, VideoNoteError> {
    authorize_video_note_send(state, room_id, sender).await?;
    // TG-204: the same topic gate as every other send path.
    let topic_id = state
        .resolve_post_topic(room_id, sender.id, upload.topic_id)
        .await
        .map_err(|error| VideoNoteError::from(VoiceError::from_topic(error)))?;
    if let Some(thumbnail) = &upload.thumbnail {
        validate_thumbnail(thumbnail)?;
    }
    let probed = probe::probe(&upload.bytes)
        .filter(|probed| probe::has_video_track(&upload.bytes, probed.container))
        .ok_or(VideoNoteError::UnsupportedMedia)?;
    let (mime_type, extension) = match probed.container {
        probe::Container::WebM => ("video/webm", "webm"),
        probe::Container::Mp4 => ("video/mp4", "mp4"),
        probe::Container::Ogg => return Err(VideoNoteError::UnsupportedMedia),
    };
    let (duration_ms, duration_source) = match (probed.duration_ms, upload.client_duration_ms) {
        (Some(ms), _) => (ms, DurationSource::Container),
        (None, Some(ms)) => (ms, DurationSource::Client),
        (None, None) => return Err(VideoNoteError::Invalid("missing_duration")),
    };
    if duration_ms == 0 || duration_ms > MAX_VIDEO_NOTE_DURATION_MS {
        return Err(VideoNoteError::Invalid("invalid_duration"));
    }

    let _permit = state
        .work_queue()
        .upload()
        .await
        .map_err(|_| VideoNoteError::Unavailable)?;
    let size_bytes = i64::try_from(upload.bytes.len()).map_err(|_| VideoNoteError::TooLarge)?;
    let mut staged = state.attachment_store().begin().await?;
    staged.write(&upload.bytes).await?;
    let content_hash = state.attachment_store().hash_staged(&mut staged).await?;
    let _guard = state.content_hash_locks().lock(&content_hash).await;
    let storage_key = match state.healthy_storage_key(&content_hash).await? {
        Some(key) => key,
        None => {
            state
                .attachment_store()
                .commit(staged, &content_hash)
                .await?;
            content_hash.clone()
        }
    };
    let display_name = state.resolve_display_name(room_id, sender).await;
    let message = state
        .insert_video_note_message(
            room_id,
            sender,
            &display_name,
            NewVideoNote {
                file_name: format!("video_note.{extension}"),
                mime_type: mime_type.to_string(),
                size_bytes,
                content_hash,
                storage_key,
                duration_ms,
                duration_source,
                thumbnail: upload.thumbnail,
                reply_to: upload.reply_to,
                topic_id,
            },
        )
        .await?;
    state.invalidate_message_cache(room_id).await;
    state
        .broadcast(room_id, stored_message_to_chat(message.clone()))
        .await;
    Ok(message)
}

/// Mark `message_id` as watched by `user`. Idempotent; the sender's own call is a no-op. The
/// first view emits `voice_listened` to the viewer's and the sender's connections only.
pub async fn mark_video_note_listened(
    state: &SharedState,
    message_id: Uuid,
    user: &User,
) -> Result<(), VideoNoteError> {
    let (room_id, sender_id) = state.video_note_listen_target(message_id, user.id).await?;
    if sender_id == Some(user.id) {
        return Ok(());
    }
    if let Some(listened) = state
        .record_video_note_listen(message_id, room_id, sender_id, user.id)
        .await?
    {
        state
            .broadcast(
                listened.room_id,
                ChatMessage::VoiceListened {
                    message_id: listened.message_id,
                    user_id: listened.user_id,
                    sender_id: listened.sender_id,
                },
            )
            .await;
    }
    Ok(())
}
