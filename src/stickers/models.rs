//! Sticker wire types, frozen in `docs/devlog/TG-302.md` ("Frozen interface").

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

/// The three sticker file formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum StickerFormat {
    Webp,
    Tgs,
    Webm,
}

impl StickerFormat {
    pub const fn as_str(self) -> &'static str {
        match self {
            StickerFormat::Webp => "webp",
            StickerFormat::Tgs => "tgs",
            StickerFormat::Webm => "webm",
        }
    }

    pub const fn mime_type(self) -> &'static str {
        match self {
            StickerFormat::Webp => "image/webp",
            StickerFormat::Tgs => "application/x-tgsticker",
            StickerFormat::Webm => "video/webm",
        }
    }

    /// Largest accepted file, before any decompression.
    pub const fn max_bytes(self) -> usize {
        match self {
            StickerFormat::Webp => 512 * 1024,
            StickerFormat::Tgs => 64 * 1024,
            StickerFormat::Webm => 256 * 1024,
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "webp" => Some(StickerFormat::Webp),
            "tgs" => Some(StickerFormat::Tgs),
            "webm" => Some(StickerFormat::Webm),
            _ => None,
        }
    }
}

/// What a set holds: ordinary stickers, or custom emoji (TG-304).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SetType {
    #[default]
    Regular,
    CustomEmoji,
}

impl SetType {
    pub const fn as_str(self) -> &'static str {
        match self {
            SetType::Regular => "regular",
            SetType::CustomEmoji => "custom_emoji",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "regular" => Some(SetType::Regular),
            "custom_emoji" => Some(SetType::CustomEmoji),
            _ => None,
        }
    }

    /// Telegram's per-set ceiling.
    pub const fn max_stickers(self) -> i64 {
        match self {
            SetType::Regular => 120,
            SetType::CustomEmoji => 200,
        }
    }
}

/// One sticker of a set. `file_url` is the set-catalogue capability URL.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct Sticker {
    pub id: Uuid,
    pub set_id: Uuid,
    pub emoji: String,
    pub emojis: Vec<String>,
    pub format: StickerFormat,
    pub mime_type: String,
    pub width: i64,
    pub height: i64,
    pub duration_ms: Option<i64>,
    pub size_bytes: i64,
    pub file_url: String,
}

/// A sticker set, with its live stickers in order and the caller's library state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct StickerSet {
    pub id: Uuid,
    pub short_name: String,
    pub title: String,
    pub set_type: SetType,
    pub owner_id: Option<Uuid>,
    pub stickers: Vec<Sticker>,
    pub installed: bool,
    pub archived: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// The caller's installed sets, top first. `revision` changes on every library write.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct InstalledStickerSets {
    pub revision: i64,
    pub sets: Vec<StickerSet>,
}

/// `POST /api/sticker-sets`.
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct CreateStickerSetRequest {
    pub short_name: String,
    pub title: String,
    #[serde(default)]
    pub set_type: SetType,
}

/// `PUT /api/stickers/installed`: the desired order. Installed sets missing from the list
/// keep their relative order after the listed ones; unknown ids are ignored, so a client
/// reordering from a stale list never drops a set another device just installed.
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct ReorderStickerSetsRequest {
    pub set_ids: Vec<Uuid>,
}

/// `PATCH /api/stickers/installed/:set_id`.
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct ArchiveStickerSetRequest {
    pub archived: bool,
}

/// `POST /api/chats/:id/sticker-messages`.
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct SendStickerRequest {
    pub sticker_id: Uuid,
    #[serde(default)]
    pub reply_to: Option<Uuid>,
    #[serde(default)]
    pub client_message_id: Option<Uuid>,
    /// TG-204: the forum topic to post into; absent = General. The handler replaces it with
    /// the resolved stored value (`None` = General) before `send_sticker_message`.
    #[serde(default)]
    pub topic_id: Option<Uuid>,
}

/// The `sticker` field of a sticker message in history and in the `broadcast` frame. The
/// file itself is the message's `attachment` (per-message capability URL).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct MessageSticker {
    pub sticker_id: Uuid,
    pub set_id: Uuid,
    pub set_short_name: String,
    pub emoji: String,
    pub format: StickerFormat,
    pub width: i64,
    pub height: i64,
}

/// `media_kind` value of a sticker message.
pub const MEDIA_KIND_STICKER: &str = "sticker";
