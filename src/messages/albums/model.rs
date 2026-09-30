//! Wire types and errors of the album interface (`super`).

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::models::StoredMessage;

/// Telegram's album bounds: a single medium is an ordinary message, more than ten is refused.
pub const MIN_ALBUM_ITEMS: usize = 2;
pub const MAX_ALBUM_ITEMS: usize = 10;
/// Same ceiling as every other message text (`upload_handlers::MAX_MESSAGE_CHARS`).
pub const MAX_CAPTION_CHARS: usize = 4096;

/// `POST /api/chats/{id}/albums`. Every upload session must belong to the caller, target this
/// chat, be fully received and hold a photo or a video; the album keeps this order.
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct SendAlbumRequest {
    pub upload_ids: Vec<Uuid>,
    /// Stored on the first item, like Telegram's single album caption.
    #[serde(default)]
    pub caption: String,
    /// Only the first item replies (one quote, one notification).
    #[serde(default)]
    pub reply_to: Option<Uuid>,
    #[serde(default)]
    pub is_sensitive: bool,
    /// TG-404: every item is sent silently.
    #[serde(default)]
    pub silent: bool,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct SendAlbumResponse {
    pub grouped_id: Uuid,
    /// The created messages in album order; each was also broadcast.
    pub messages: Vec<StoredMessage>,
}

/// One album item removed by `DELETE /api/chats/{id}/albums/{grouped_id}`.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct RecalledAlbumItem {
    pub message_id: Uuid,
    pub recalled_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct RecallAlbumResponse {
    pub recalled: Vec<RecalledAlbumItem>,
}

#[derive(Debug)]
pub enum AlbumError {
    /// A request field is malformed; the code is the wire string.
    Invalid(&'static str),
    Unauthorized,
    /// The caller may not send (media) in this chat.
    Forbidden,
    /// No such chat, upload or album, or not visible to the caller (never distinguished).
    NotFound,
    /// An upload of this request was already turned into a message.
    Conflict,
    Unavailable,
    Database(sqlx::Error),
    Storage(anyhow::Error),
}

impl From<sqlx::Error> for AlbumError {
    fn from(error: sqlx::Error) -> Self {
        AlbumError::Database(error)
    }
}

impl From<anyhow::Error> for AlbumError {
    fn from(error: anyhow::Error) -> Self {
        AlbumError::Storage(error)
    }
}

impl From<StatusCode> for AlbumError {
    fn from(status: StatusCode) -> Self {
        match status {
            StatusCode::UNAUTHORIZED => AlbumError::Unauthorized,
            StatusCode::FORBIDDEN => AlbumError::Forbidden,
            StatusCode::NOT_FOUND => AlbumError::NotFound,
            StatusCode::SERVICE_UNAVAILABLE => AlbumError::Unavailable,
            _ => AlbumError::Invalid("bad_request"),
        }
    }
}

impl IntoResponse for AlbumError {
    fn into_response(self) -> Response {
        let (status, code) = match self {
            AlbumError::Invalid(code) => (StatusCode::BAD_REQUEST, code),
            AlbumError::Unauthorized => (StatusCode::UNAUTHORIZED, "unauthorized"),
            AlbumError::Forbidden => (StatusCode::FORBIDDEN, "forbidden"),
            AlbumError::NotFound => (StatusCode::NOT_FOUND, "not_found"),
            AlbumError::Conflict => (StatusCode::CONFLICT, "upload_consumed"),
            AlbumError::Unavailable => (StatusCode::SERVICE_UNAVAILABLE, "busy"),
            AlbumError::Database(error) => {
                tracing::error!("album database operation failed: {error}");
                (StatusCode::INTERNAL_SERVER_ERROR, "internal")
            }
            AlbumError::Storage(error) => {
                tracing::error!("album storage operation failed: {error:#}");
                (StatusCode::INTERNAL_SERVER_ERROR, "internal")
            }
        };
        (status, Json(serde_json::json!({ "error": code }))).into_response()
    }
}

/// Photos and videos only (Telegram also groups documents and audio; this client does not).
/// SVG is script-capable and is always a file, never a medium.
pub fn is_album_media(mime_type: &str) -> bool {
    let mime = mime_type.trim().to_ascii_lowercase();
    (mime.starts_with("image/") && mime != "image/svg+xml") || mime.starts_with("video/")
}
