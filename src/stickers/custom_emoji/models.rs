//! Custom emoji wire types, frozen in `docs/devlog/TG-304.md` ("Frozen interface").

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::stickers::models::StickerFormat;

/// Most ids one resolve / status lookup accepts.
pub const MAX_LOOKUP_IDS: usize = 200;

/// One custom emoji: a sticker of a `custom_emoji` set. `file_url` is the set-catalogue
/// capability URL, exactly as for a sticker.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct CustomEmoji {
    pub id: Uuid,
    pub set_id: Uuid,
    pub set_short_name: String,
    /// The fallback Unicode emoji, also used for copy/paste and search.
    pub emoji: String,
    pub format: StickerFormat,
    pub width: i64,
    pub height: i64,
    pub file_url: String,
}

/// An account's emoji status, shown next to its name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct EmojiStatus {
    pub user_id: Uuid,
    pub custom_emoji_id: Uuid,
    pub expires_at: Option<DateTime<Utc>>,
    pub emoji: CustomEmoji,
}

/// `PUT /api/users/me/emoji-status`. `expires_at` absent = until cleared.
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct SetEmojiStatusRequest {
    pub custom_emoji_id: Uuid,
    #[serde(default)]
    pub expires_at: Option<DateTime<Utc>>,
}
