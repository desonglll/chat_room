//! Creating sticker sets and managing their stickers. Only a set's owner edits it.

use chrono::Utc;
use uuid::Uuid;

use super::catalogue::{SetRow, StickerScope};
use super::errors::StickerError;
use super::models::{CreateStickerSetRequest, SetType, Sticker, StickerSet};
use super::validation::ValidatedSticker;
use crate::state::{with_pool, AppState};

const MAX_SHORT_NAME_CHARS: usize = 64;
const MAX_TITLE_CHARS: usize = 64;
const MAX_EMOJIS: usize = 20;
const MAX_EMOJI_CHARS: usize = 16;

/// Lowercase canonical short name: a letter, then letters, digits or underscores.
pub fn normalize_short_name(value: &str) -> Option<String> {
    let name = value.trim().to_ascii_lowercase();
    let mut chars = name.chars();
    let valid = chars.next().is_some_and(|first| first.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        && name.len() <= MAX_SHORT_NAME_CHARS
        && !name.contains("__");
    valid.then_some(name)
}

/// Space-separated emoji, deduplicated in order. A token must contain a non-ASCII symbol
/// (keycaps like `1️⃣` start with an ASCII digit, so ASCII alone is not refused per char).
pub fn parse_emoji_list(value: &str) -> Option<Vec<String>> {
    let mut emojis: Vec<String> = Vec::new();
    for token in value.split_whitespace() {
        let plausible = token.chars().count() <= MAX_EMOJI_CHARS
            && token.chars().any(|c| u32::from(c) >= 0xa9)
            && !token
                .chars()
                .any(|c| c.is_control() || c.is_ascii_alphabetic());
        if !plausible {
            return None;
        }
        if !emojis.iter().any(|existing| existing == token) {
            emojis.push(token.to_string());
        }
    }
    (!emojis.is_empty() && emojis.len() <= MAX_EMOJIS).then_some(emojis)
}

/// A validated file already committed to the object store.
pub struct StoredStickerFile {
    pub validated: ValidatedSticker,
    pub size_bytes: i64,
    pub content_hash: String,
    pub storage_key: String,
}

impl AppState {
    pub async fn create_sticker_set(
        &self,
        owner_id: Uuid,
        request: &CreateStickerSetRequest,
    ) -> Result<StickerSet, StickerError> {
        let short_name = normalize_short_name(&request.short_name)
            .ok_or(StickerError::Invalid("invalid_short_name"))?;
        let title = request.title.trim();
        if title.is_empty() || title.chars().count() > MAX_TITLE_CHARS {
            return Err(StickerError::Invalid("invalid_title"));
        }
        let now = Utc::now();
        let row = SetRow {
            id: Uuid::new_v4(),
            short_name,
            title: title.to_string(),
            set_type: request.set_type.as_str().to_string(),
            owner_id: Some(owner_id),
            created_at: now,
            updated_at: now,
        };
        let inserted = with_pool!(self, |pool| {
            sqlx::query(
                "INSERT INTO sticker_sets \
                 (id, short_name, title, set_type, owner_id, created_at, updated_at) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7) ON CONFLICT (short_name) DO NOTHING",
            )
            .bind(row.id)
            .bind(&row.short_name)
            .bind(&row.title)
            .bind(&row.set_type)
            .bind(owner_id)
            .bind(now)
            .bind(now)
            .execute(pool)
            .await
            .map(|result| result.rows_affected() > 0)
        })?;
        if !inserted {
            return Err(StickerError::Conflict("short_name_taken"));
        }
        Ok(row.into_set(Vec::new(), None))
    }

    /// The set row, if `owner_id` owns it; `Forbidden` when someone else does.
    pub(crate) async fn owned_sticker_set(
        &self,
        short_name: &str,
        owner_id: Uuid,
    ) -> Result<SetRow, StickerError> {
        let row = self
            .sticker_set_row_by_short_name(short_name)
            .await?
            .ok_or(StickerError::NotFound("sticker_set_not_found"))?;
        if row.owner_id != Some(owner_id) {
            return Err(StickerError::Forbidden);
        }
        Ok(row)
    }

    /// Append a sticker to an owned set. The set row is written first so concurrent adds to
    /// one set serialise and never share a position or overshoot the set ceiling.
    pub(crate) async fn add_sticker(
        &self,
        set: &SetRow,
        emojis: &[String],
        file: StoredStickerFile,
    ) -> Result<Sticker, StickerError> {
        let set_type = set.set_type();
        let id = Uuid::new_v4();
        let access_key = Uuid::new_v4();
        let now = Utc::now();
        let format = file.validated.format;
        let result: Result<(), StickerError> = with_pool!(self, |pool| {
            async {
                let mut tx = pool.begin().await?;
                sqlx::query("UPDATE sticker_sets SET updated_at = $1 WHERE id = $2")
                    .bind(now)
                    .bind(set.id)
                    .execute(&mut *tx)
                    .await?;
                let (count, next): (i64, i64) = sqlx::query_as(
                    "SELECT COUNT(*), COALESCE(MAX(position), -1) + 1 FROM stickers \
                     WHERE set_id = $1 AND removed_at IS NULL",
                )
                .bind(set.id)
                .fetch_one(&mut *tx)
                .await?;
                if count >= set_type.max_stickers() {
                    return Err(StickerError::Invalid("set_full"));
                }
                sqlx::query(
                    "INSERT INTO stickers (id, set_id, position, emoji, format, mime_type, \
                     width, height, duration_ms, size_bytes, content_hash, storage_key, \
                     access_key, created_at) \
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)",
                )
                .bind(id)
                .bind(set.id)
                .bind(next)
                .bind(&emojis[0])
                .bind(format.as_str())
                .bind(format.mime_type())
                .bind(i64::from(file.validated.width))
                .bind(i64::from(file.validated.height))
                .bind(file.validated.duration_ms.map(i64::from))
                .bind(file.size_bytes)
                .bind(&file.content_hash)
                .bind(&file.storage_key)
                .bind(access_key)
                .bind(now)
                .execute(&mut *tx)
                .await?;
                for (position, emoji) in emojis.iter().enumerate() {
                    sqlx::query(
                        "INSERT INTO sticker_emojis (sticker_id, emoji, position) \
                         VALUES ($1, $2, $3)",
                    )
                    .bind(id)
                    .bind(emoji)
                    .bind(position as i64)
                    .execute(&mut *tx)
                    .await?;
                }
                if set_type == SetType::CustomEmoji {
                    sqlx::query(
                        "INSERT INTO custom_emoji (sticker_id, set_id, emoji, created_at) \
                         VALUES ($1, $2, $3, $4)",
                    )
                    .bind(id)
                    .bind(set.id)
                    .bind(&emojis[0])
                    .bind(now)
                    .execute(&mut *tx)
                    .await?;
                }
                tx.commit().await?;
                Ok(())
            }
            .await
        });
        result?;
        self.load_stickers(&StickerScope::Set(set.id))
            .await?
            .into_iter()
            .find(|sticker| sticker.id == id)
            .ok_or(StickerError::NotFound("sticker_not_found"))
    }

    /// Soft-delete: the row stays so sent messages keep resolving their set and emoji,
    /// and recents/favorites stop listing it immediately (loaders skip removed stickers).
    pub(crate) async fn remove_sticker(
        &self,
        set: &SetRow,
        sticker_id: Uuid,
    ) -> Result<(), StickerError> {
        let now = Utc::now();
        let removed = with_pool!(self, |pool| {
            sqlx::query(
                "UPDATE stickers SET removed_at = $1 \
                 WHERE id = $2 AND set_id = $3 AND removed_at IS NULL",
            )
            .bind(now)
            .bind(sticker_id)
            .bind(set.id)
            .execute(pool)
            .await
            .map(|result| result.rows_affected() > 0)
        })?;
        if !removed {
            return Err(StickerError::NotFound("sticker_not_found"));
        }
        with_pool!(self, |pool| {
            sqlx::query("UPDATE sticker_sets SET updated_at = $1 WHERE id = $2")
                .bind(now)
                .bind(set.id)
                .execute(pool)
                .await
                .map(|_| ())
        })?;
        Ok(())
    }
}
