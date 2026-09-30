//! Resolving custom emoji ids to files, and the caller's installed custom emoji sets.
//!
//! Authorization is the sticker catalogue's: sets are public by short name to any signed-in
//! account (Telegram semantics — a recipient sees a custom emoji without installing its
//! set), and the file is served behind the sticker's own capability key.

use std::collections::HashMap;

use sqlx::{FromRow, QueryBuilder};
use uuid::Uuid;

use super::models::CustomEmoji;
use crate::state::{with_pool, AppState};
use crate::stickers::catalogue::sticker_file_url;
use crate::stickers::models::{InstalledStickerSets, SetType, StickerFormat};

#[derive(FromRow)]
struct CustomEmojiRow {
    id: Uuid,
    set_id: Uuid,
    set_short_name: String,
    emoji: String,
    format: String,
    width: i64,
    height: i64,
    access_key: Uuid,
}

impl CustomEmojiRow {
    fn into_custom_emoji(self) -> Option<CustomEmoji> {
        Some(CustomEmoji {
            format: StickerFormat::parse(&self.format)?,
            file_url: sticker_file_url(self.id, self.access_key),
            id: self.id,
            set_id: self.set_id,
            set_short_name: self.set_short_name,
            emoji: self.emoji,
            width: self.width,
            height: self.height,
        })
    }
}

impl AppState {
    /// The live custom emoji among `ids`, keyed by id. Unknown ids, ordinary stickers and
    /// emoji removed from their set are absent — the client shows the fallback emoji.
    pub async fn resolve_custom_emoji(
        &self,
        ids: &[Uuid],
    ) -> Result<HashMap<Uuid, CustomEmoji>, sqlx::Error> {
        if ids.is_empty() {
            return Ok(HashMap::new());
        }
        let rows: Vec<CustomEmojiRow> = with_pool!(self, |pool| {
            let mut query = QueryBuilder::new(
                "SELECT stickers.id, stickers.set_id, sticker_sets.short_name AS set_short_name, \
                 stickers.emoji, stickers.format, stickers.width, stickers.height, \
                 stickers.access_key FROM custom_emoji \
                 JOIN stickers ON stickers.id = custom_emoji.sticker_id \
                 JOIN sticker_sets ON sticker_sets.id = stickers.set_id \
                 WHERE stickers.removed_at IS NULL AND custom_emoji.sticker_id IN (",
            );
            {
                let mut values = query.separated(", ");
                for id in ids {
                    values.push_bind(*id);
                }
            }
            query.push(")");
            query.build_query_as().fetch_all(pool).await
        })?;
        Ok(rows
            .into_iter()
            .filter_map(CustomEmojiRow::into_custom_emoji)
            .map(|emoji| (emoji.id, emoji))
            .collect())
    }

    /// The caller's library restricted to custom emoji sets, in library order. Installing,
    /// archiving and reordering stay on the shared `/api/stickers/installed` endpoints.
    pub async fn installed_custom_emoji_sets(
        &self,
        user_id: Uuid,
    ) -> Result<InstalledStickerSets, sqlx::Error> {
        let mut library = self.installed_sticker_sets(user_id).await?;
        library
            .sets
            .retain(|set| set.set_type == SetType::CustomEmoji);
        Ok(library)
    }
}
