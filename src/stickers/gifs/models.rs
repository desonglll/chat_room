//! Wire types of the GIF domain (TG-305). The contract is frozen in `docs/devlog/TG-305.md`.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// `messages.media_kind` of a GIF message.
pub const MEDIA_KIND_GIF: &str = "gif";
/// Saved GIFs per account; saving beyond it evicts the least recently used, as in Telegram.
pub const SAVED_GIF_LIMIT: i64 = 200;
/// Default and largest page of `GET /api/gifs/recent`.
pub const RECENT_GIF_LIMIT: i64 = 60;

/// One entry of the account's saved GIFs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedGif {
    pub id: Uuid,
    pub mime_type: String,
    pub size_bytes: i64,
    /// Geometry when the server read it from an uploaded file's header, else `null` (the
    /// client measures the loaded media).
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub duration_ms: Option<i64>,
    /// Capability URL of the account's own copy: `/api/gifs/saved/:id/file?key=`.
    pub file_url: String,
    pub saved_at: DateTime<Utc>,
    pub used_at: DateTime<Utc>,
}

/// A GIF recently sent into a chat the account can read (newest first, one per file).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecentGif {
    pub message_id: Uuid,
    pub room_id: Uuid,
    pub mime_type: String,
    pub size_bytes: i64,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub duration_ms: Option<i64>,
    /// The message attachment's own capability URL, exactly what history reveals.
    pub file_url: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct SaveGifRequest {
    pub message_id: Uuid,
}

/// `POST /api/chats/:id/gif-messages`: exactly one of `saved_gif_id` / `message_id`.
#[derive(Debug, Deserialize)]
pub struct SendGifRequest {
    #[serde(default)]
    pub saved_gif_id: Option<Uuid>,
    #[serde(default)]
    pub message_id: Option<Uuid>,
    #[serde(default)]
    pub reply_to: Option<Uuid>,
    /// TG-204: the forum topic to post into; absent = General.
    #[serde(default)]
    pub topic_id: Option<Uuid>,
    #[serde(default)]
    pub client_message_id: Option<Uuid>,
}

/// Query of the raw-body upload `POST /api/chats/:id/gif-messages/upload`.
#[derive(Debug, Default, Deserialize)]
pub struct UploadGifQuery {
    #[serde(default)]
    pub reply_to: Option<Uuid>,
    /// TG-204: the forum topic to post into; absent = General.
    #[serde(default)]
    pub topic_id: Option<Uuid>,
    #[serde(default)]
    pub client_message_id: Option<Uuid>,
}

#[derive(Debug, Default, Deserialize)]
pub struct RecentGifQuery {
    #[serde(default)]
    pub limit: Option<i64>,
}
