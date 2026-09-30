//! Video note wire type, limits, thumbnail validation and the domain error.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use base64::Engine;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::attachments::voice::model::VoiceError;

/// `messages.media_kind` of a round video message.
pub const MEDIA_KIND_VIDEO_NOTE: &str = "video_note";

/// Telegram's cap on a video note is one minute. The recorder stops itself at 60 s; the
/// container's last frame may end a little later, so framing gets one second of grace.
pub const MAX_VIDEO_NOTE_DURATION_MS: u32 = 61_000;
/// Largest accepted file (also bounded by the deployment's upload limit). The recorder
/// writes ~1 Mbit/s, so a full minute is ~8 MiB.
pub const MAX_VIDEO_NOTE_BYTES: usize = 16 * 1024 * 1024;
/// Largest accepted thumbnail: a small JPEG inlined into every history page that holds it.
pub const MAX_THUMBNAIL_BYTES: usize = 16 * 1024;

/// The playback projection of a video note, carried as `StoredMessage::video_note` and the
/// `broadcast` frame's `video_note`. Present exactly when `media_kind == "video_note"` and the
/// viewer can see the attachment.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct VideoNote {
    /// Playback length, from the container when it carries one.
    pub duration_ms: u32,
    /// Base64 of a small JPEG (the recorder's first frame), or `null`.
    #[serde(default)]
    pub thumbnail: Option<String>,
    /// Per viewer, like `VoiceNote::listened`: for the sender, some other member watched it;
    /// for everyone else, this viewer watched it. Chat-wide frames carry `false`.
    #[serde(default)]
    pub listened: bool,
}

pub(crate) fn encode_thumbnail(bytes: Option<Vec<u8>>) -> Option<String> {
    bytes.map(|bytes| base64::engine::general_purpose::STANDARD.encode(bytes))
}

/// A thumbnail must be a JPEG (SOI marker) no larger than [`MAX_THUMBNAIL_BYTES`]. It is only
/// ever shown through an `<img>` as `data:image/jpeg`, never served as a file.
pub fn validate_thumbnail(bytes: &[u8]) -> Result<(), VideoNoteError> {
    if bytes.len() > MAX_THUMBNAIL_BYTES || !bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Err(VideoNoteError::Invalid("invalid_thumbnail"));
    }
    Ok(())
}

#[derive(Debug)]
pub enum VideoNoteError {
    /// A request field is malformed; the code is the wire string.
    Invalid(&'static str),
    /// Not a WebM/MP4 file with a video track.
    UnsupportedMedia,
    TooLarge,
    Unauthorized,
    Forbidden,
    /// TG-505: the private-chat peer's "voice messages" rule (which covers video messages).
    Restricted,
    NotFound,
    Unavailable,
    Database(sqlx::Error),
    Storage(anyhow::Error),
}

impl From<sqlx::Error> for VideoNoteError {
    fn from(error: sqlx::Error) -> Self {
        VideoNoteError::Database(error)
    }
}

impl From<anyhow::Error> for VideoNoteError {
    fn from(error: anyhow::Error) -> Self {
        VideoNoteError::Storage(error)
    }
}

/// The shared pieces (session, send authorization) come from the voice module.
impl From<VoiceError> for VideoNoteError {
    fn from(error: VoiceError) -> Self {
        match error {
            VoiceError::Invalid(code) => VideoNoteError::Invalid(code),
            VoiceError::UnsupportedMedia => VideoNoteError::UnsupportedMedia,
            VoiceError::TooLarge => VideoNoteError::TooLarge,
            VoiceError::Unauthorized => VideoNoteError::Unauthorized,
            VoiceError::Forbidden => VideoNoteError::Forbidden,
            VoiceError::Restricted => VideoNoteError::Restricted,
            VoiceError::NotFound => VideoNoteError::NotFound,
            VoiceError::Unavailable => VideoNoteError::Unavailable,
            VoiceError::Database(error) => VideoNoteError::Database(error),
            VoiceError::Storage(error) => VideoNoteError::Storage(error),
        }
    }
}

impl From<StatusCode> for VideoNoteError {
    fn from(status: StatusCode) -> Self {
        VoiceError::from(status).into()
    }
}

impl IntoResponse for VideoNoteError {
    fn into_response(self) -> Response {
        let (status, code) = match self {
            VideoNoteError::Invalid(code) => (StatusCode::BAD_REQUEST, code),
            VideoNoteError::UnsupportedMedia => {
                (StatusCode::UNSUPPORTED_MEDIA_TYPE, "unsupported_video")
            }
            VideoNoteError::TooLarge => (StatusCode::PAYLOAD_TOO_LARGE, "too_large"),
            VideoNoteError::Unauthorized => (StatusCode::UNAUTHORIZED, "unauthorized"),
            VideoNoteError::Forbidden => (StatusCode::FORBIDDEN, "forbidden"),
            VideoNoteError::Restricted => (StatusCode::FORBIDDEN, "voice_messages_restricted"),
            VideoNoteError::NotFound => (StatusCode::NOT_FOUND, "not_found"),
            VideoNoteError::Unavailable => (StatusCode::SERVICE_UNAVAILABLE, "busy"),
            VideoNoteError::Database(error) => {
                tracing::error!("video note database operation failed: {error}");
                (StatusCode::INTERNAL_SERVER_ERROR, "internal")
            }
            VideoNoteError::Storage(error) => {
                tracing::error!("video note storage operation failed: {error:#}");
                (StatusCode::INTERNAL_SERVER_ERROR, "internal")
            }
        };
        (status, Json(serde_json::json!({ "error": code }))).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thumbnails_must_be_small_jpegs() {
        assert!(validate_thumbnail(&[0xFF, 0xD8, 0xFF, 0xE0, 0, 0]).is_ok());
        assert!(validate_thumbnail(b"\x89PNG\r\n\x1a\n").is_err());
        assert!(validate_thumbnail(b"<svg onload=alert(1)>").is_err());
        let mut large = vec![0xFF, 0xD8, 0xFF];
        large.resize(MAX_THUMBNAIL_BYTES + 1, 0);
        assert!(validate_thumbnail(&large).is_err());
    }

    #[test]
    fn the_thumbnail_rides_the_wire_as_base64() {
        assert_eq!(
            encode_thumbnail(Some(vec![0xFF, 0xD8, 0xFF])),
            Some("/9j/".into())
        );
        assert_eq!(encode_thumbnail(None), None);
    }
}
