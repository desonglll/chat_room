//! Voice messages (TG-401).
//!
//! A voice message is an attachment message with `media_kind = 'voice'` plus a
//! `voice_notes` row (duration, 100-sample waveform), so it flows through the normal history,
//! realtime and download paths and only gains `StoredMessage::voice`. The listened ("played")
//! state is one `voice_listens` row per listener; a first listen emits `voice_listened` to the
//! listener's and the sender's own connections.
//!
//! The functions below are the module interface; `handlers` only translates HTTP.
//! Contract: `docs/devlog/TG-401.md`, Frozen interface.

pub(crate) mod handlers;
pub mod model;
pub mod probe;
mod store;

use std::sync::Arc;

use axum::{extract::DefaultBodyLimit, routing::post, Router};
use uuid::Uuid;

use crate::chats::ChatType;
use crate::models::{ChatMessage, StoredMessage, User};
use crate::realtime::protocol::stored_message_to_chat;
use crate::state::{AppState, SharedState};
use model::{DurationSource, VoiceError, MAX_VOICE_BYTES, MAX_VOICE_DURATION_MS, WAVEFORM_SAMPLES};
use store::NewVoice;

/// The voice routes. Canonical prefix only: the deprecated `/api/rooms` alias serves frozen
/// clients that know no voice messages.
pub fn routes(multipart_body_limit: usize) -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/api/chats/:id/voice",
            post(handlers::send).layer(DefaultBodyLimit::max(
                multipart_body_limit.min(MAX_VOICE_BYTES + 64 * 1024),
            )),
        )
        .route(
            "/api/messages/:message_id/voice/listened",
            post(handlers::listened),
        )
}

/// Forwarding copies the voice projection with the message (Telegram semantics: a forwarded
/// voice message is a voice message, unlistened). Both run in the forward transaction and are
/// no-ops for a source that is not a voice message.
/// Binds: `$1` new message id, `$2` source message id, `$3` created_at.
pub(crate) const FORWARD_COPY_VOICE_NOTE: &str = "INSERT INTO voice_notes \
    (message_id, duration_ms, waveform, duration_source, created_at) \
    SELECT $1, duration_ms, waveform, duration_source, $3 FROM voice_notes WHERE message_id = $2";
/// Binds: `$1` new message id, `$2` source message id.
pub(crate) const FORWARD_MARK_VOICE: &str = "UPDATE messages SET media_kind = 'voice' \
    WHERE id = $1 AND EXISTS (SELECT 1 FROM voice_notes WHERE message_id = $2)";

/// A recorded voice message as the handler received it.
pub struct VoiceUpload {
    pub bytes: Vec<u8>,
    /// The recorder's own measurement; used only when the container carries no duration.
    pub client_duration_ms: Option<u32>,
    /// Already validated by [`model::parse_waveform`].
    pub waveform: Vec<u8>,
    pub reply_to: Option<Uuid>,
}

/// Who may send a voice message into `room_id`: an active member holding `message.send`
/// (`message.post` in a channel), and — in a private chat — only when the peer's TG-505
/// `voice_messages` rule admits the sender.
pub async fn authorize_voice_send(
    state: &SharedState,
    room_id: Uuid,
    sender: &User,
) -> Result<(), VoiceError> {
    let chat = state.chat(room_id).await.ok_or(VoiceError::NotFound)?;
    if !state.is_chat_participant(room_id, sender.id).await? {
        return Err(VoiceError::NotFound);
    }
    let may_send = state
        .has_chat_permission(room_id, sender.id, "message.send")
        .await?
        || state
            .has_chat_permission(room_id, sender.id, "message.post")
            .await?;
    if !may_send {
        return Err(VoiceError::Forbidden);
    }
    if chat.chat_type == ChatType::Private {
        if let Some(peer) = state.private_chat_peer_id(room_id, sender.id).await? {
            if !state.voice_message_allowed(sender.id, peer).await? {
                return Err(VoiceError::Restricted);
            }
        }
    }
    Ok(())
}

/// Validate, store and broadcast one voice message. The container is sniffed from the bytes
/// (never trusted from the client), and the duration is read from it when it carries one.
pub async fn send_voice(
    state: &SharedState,
    room_id: Uuid,
    sender: &User,
    upload: VoiceUpload,
) -> Result<StoredMessage, VoiceError> {
    authorize_voice_send(state, room_id, sender).await?;
    if upload.waveform.len() != WAVEFORM_SAMPLES {
        return Err(VoiceError::Invalid("invalid_waveform"));
    }
    let probed = probe::probe(&upload.bytes).ok_or(VoiceError::UnsupportedMedia)?;
    let (duration_ms, duration_source) = match (probed.duration_ms, upload.client_duration_ms) {
        (Some(ms), _) => (ms, DurationSource::Container),
        (None, Some(ms)) => (ms, DurationSource::Client),
        (None, None) => return Err(VoiceError::Invalid("missing_duration")),
    };
    if duration_ms == 0 || duration_ms > MAX_VOICE_DURATION_MS {
        return Err(VoiceError::Invalid("invalid_duration"));
    }

    let _permit = state
        .work_queue()
        .upload()
        .await
        .map_err(|_| VoiceError::Unavailable)?;
    let size_bytes = i64::try_from(upload.bytes.len()).map_err(|_| VoiceError::TooLarge)?;
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
        .insert_voice_message(
            room_id,
            sender,
            &display_name,
            NewVoice {
                file_name: format!("voice.{}", probed.container.extension()),
                mime_type: probed.container.mime_type().to_string(),
                size_bytes,
                content_hash,
                storage_key,
                duration_ms,
                duration_source,
                waveform: upload.waveform,
                reply_to: upload.reply_to,
            },
        )
        .await?;
    state.invalidate_message_cache(room_id).await;
    state
        .broadcast(room_id, stored_message_to_chat(message.clone()))
        .await;
    Ok(message)
}

/// Mark `message_id` as played by `user`. Idempotent; the sender's own call is a no-op.
/// The first listen emits `voice_listened`, delivered only to the listener's and the
/// sender's connections (`frame_visible_to`).
pub async fn mark_voice_listened(
    state: &SharedState,
    message_id: Uuid,
    user: &User,
) -> Result<(), VoiceError> {
    let (room_id, sender_id) = state.voice_listen_target(message_id, user.id).await?;
    if sender_id == Some(user.id) {
        return Ok(());
    }
    if let Some(listened) = state
        .record_voice_listen(message_id, room_id, sender_id, user.id)
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
