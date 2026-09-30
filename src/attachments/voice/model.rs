//! Voice message wire types, limits, the waveform codec and the domain error.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// `messages.media_kind` of a voice message.
pub const MEDIA_KIND_VOICE: &str = "voice";

/// Telegram's waveform: exactly this many samples…
pub const WAVEFORM_SAMPLES: usize = 100;
/// …of five bits each, so every sample is in `0..=WAVEFORM_MAX`.
pub const WAVEFORM_MAX: u8 = 31;
/// Bytes of a packed waveform: `ceil(100 * 5 / 8)`.
pub const WAVEFORM_PACKED_BYTES: usize = (WAVEFORM_SAMPLES * 5).div_ceil(8);

/// Longest accepted voice message. Telegram has no hard cap; one hour bounds the file.
pub const MAX_VOICE_DURATION_MS: u32 = 60 * 60 * 1000;
/// Largest accepted voice file (also bounded by the deployment's upload limit). One hour
/// of Opus at the recorder's 32 kbit/s is ~14 MiB.
pub const MAX_VOICE_BYTES: usize = 24 * 1024 * 1024;

/// The playback projection of a voice message, carried as `StoredMessage::voice` and the
/// `broadcast` frame's `voice`. Present exactly when `media_kind == "voice"` and the viewer can
/// see the attachment (a recalled message loses both).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct VoiceNote {
    /// Playback length, from the container when it carries one (see `duration_source`).
    pub duration_ms: u32,
    /// `WAVEFORM_SAMPLES` peaks in `0..=WAVEFORM_MAX`, evenly spread over `duration_ms`.
    pub waveform: Vec<u8>,
    /// Per viewer. For the sender: some other member has played it. For everyone else: this
    /// viewer has played it. Chat-wide frames (a new message) always carry `false`.
    #[serde(default)]
    pub listened: bool,
}

/// Where a stored duration came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DurationSource {
    Container,
    Client,
}

impl DurationSource {
    pub fn as_str(self) -> &'static str {
        match self {
            DurationSource::Container => "container",
            DurationSource::Client => "client",
        }
    }
}

/// `voice_listened` frame audience: the listener's own connections and the sender's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Listened {
    pub message_id: uuid::Uuid,
    pub room_id: uuid::Uuid,
    pub user_id: uuid::Uuid,
    pub sender_id: Option<uuid::Uuid>,
}

#[derive(Debug)]
pub enum VoiceError {
    /// A request field is malformed; the code is the wire string.
    Invalid(&'static str),
    /// The file is not an Ogg/Opus, WebM or MP4 audio container.
    UnsupportedMedia,
    TooLarge,
    Unauthorized,
    /// The caller may not send messages in this chat.
    Forbidden,
    /// TG-505: the private-chat peer does not accept voice messages from the caller.
    Restricted,
    /// No such chat/message, or not visible to the caller (never distinguished).
    NotFound,
    Unavailable,
    Database(sqlx::Error),
    Storage(anyhow::Error),
}

impl From<sqlx::Error> for VoiceError {
    fn from(error: sqlx::Error) -> Self {
        VoiceError::Database(error)
    }
}

impl From<anyhow::Error> for VoiceError {
    fn from(error: anyhow::Error) -> Self {
        VoiceError::Storage(error)
    }
}

impl From<StatusCode> for VoiceError {
    fn from(status: StatusCode) -> Self {
        match status {
            StatusCode::UNAUTHORIZED => VoiceError::Unauthorized,
            StatusCode::FORBIDDEN => VoiceError::Forbidden,
            StatusCode::NOT_FOUND => VoiceError::NotFound,
            StatusCode::PAYLOAD_TOO_LARGE => VoiceError::TooLarge,
            StatusCode::SERVICE_UNAVAILABLE => VoiceError::Unavailable,
            StatusCode::BAD_REQUEST => VoiceError::Invalid("bad_request"),
            _ => VoiceError::Storage(anyhow::anyhow!("voice request failed with {status}")),
        }
    }
}

impl IntoResponse for VoiceError {
    fn into_response(self) -> Response {
        let (status, code) = match self {
            VoiceError::Invalid(code) => (StatusCode::BAD_REQUEST, code),
            VoiceError::UnsupportedMedia => {
                (StatusCode::UNSUPPORTED_MEDIA_TYPE, "unsupported_audio")
            }
            VoiceError::TooLarge => (StatusCode::PAYLOAD_TOO_LARGE, "too_large"),
            VoiceError::Unauthorized => (StatusCode::UNAUTHORIZED, "unauthorized"),
            VoiceError::Forbidden => (StatusCode::FORBIDDEN, "forbidden"),
            VoiceError::Restricted => (StatusCode::FORBIDDEN, "voice_messages_restricted"),
            VoiceError::NotFound => (StatusCode::NOT_FOUND, "not_found"),
            VoiceError::Unavailable => (StatusCode::SERVICE_UNAVAILABLE, "busy"),
            VoiceError::Database(error) => {
                tracing::error!("voice database operation failed: {error}");
                (StatusCode::INTERNAL_SERVER_ERROR, "internal")
            }
            VoiceError::Storage(error) => {
                tracing::error!("voice storage operation failed: {error:#}");
                (StatusCode::INTERNAL_SERVER_ERROR, "internal")
            }
        };
        (status, Json(serde_json::json!({ "error": code }))).into_response()
    }
}

/// Parse the recorder's `waveform` field: comma-separated integers, exactly
/// `WAVEFORM_SAMPLES` of them, each in `0..=WAVEFORM_MAX`.
pub fn parse_waveform(field: &str) -> Result<Vec<u8>, VoiceError> {
    let samples = field
        .split(',')
        .map(|value| value.trim().parse::<u8>())
        .collect::<Result<Vec<u8>, _>>()
        .map_err(|_| VoiceError::Invalid("invalid_waveform"))?;
    if samples.len() != WAVEFORM_SAMPLES {
        return Err(VoiceError::Invalid("invalid_waveform"));
    }
    if samples.iter().any(|sample| *sample > WAVEFORM_MAX) {
        return Err(VoiceError::Invalid("invalid_waveform"));
    }
    Ok(samples)
}

/// Pack 5-bit samples little-endian: sample `i` occupies bits `5i..5i+5` of the byte string.
pub fn pack_waveform(samples: &[u8]) -> Vec<u8> {
    let mut packed = vec![0u8; (samples.len() * 5).div_ceil(8)];
    for (index, sample) in samples.iter().enumerate() {
        let bit = index * 5;
        let value = u16::from(sample & WAVEFORM_MAX) << (bit % 8);
        packed[bit / 8] |= value as u8;
        if let Some(next) = packed.get_mut(bit / 8 + 1) {
            *next |= (value >> 8) as u8;
        }
    }
    packed
}

/// Inverse of [`pack_waveform`] for `WAVEFORM_SAMPLES` samples. Short input yields zeros.
pub fn unpack_waveform(packed: &[u8]) -> Vec<u8> {
    (0..WAVEFORM_SAMPLES)
        .map(|index| {
            let bit = index * 5;
            let low = u16::from(packed.get(bit / 8).copied().unwrap_or(0));
            let high = u16::from(packed.get(bit / 8 + 1).copied().unwrap_or(0));
            (((high << 8 | low) >> (bit % 8)) as u8) & WAVEFORM_MAX
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn waveform_round_trips_through_the_packed_form() {
        let samples: Vec<u8> = (0..WAVEFORM_SAMPLES).map(|i| (i * 7 % 32) as u8).collect();
        let packed = pack_waveform(&samples);
        assert_eq!(packed.len(), WAVEFORM_PACKED_BYTES);
        assert_eq!(unpack_waveform(&packed), samples);
    }

    #[test]
    fn waveform_field_is_validated_for_length_and_range() {
        let ok = vec!["31"; WAVEFORM_SAMPLES].join(",");
        assert_eq!(parse_waveform(&ok).unwrap(), vec![31; WAVEFORM_SAMPLES]);
        let short = vec!["1"; WAVEFORM_SAMPLES - 1].join(",");
        assert!(parse_waveform(&short).is_err());
        let high = format!("32,{}", vec!["0"; WAVEFORM_SAMPLES - 1].join(","));
        assert!(parse_waveform(&high).is_err());
        assert!(parse_waveform("a,b").is_err());
        assert!(parse_waveform("").is_err());
    }
}
