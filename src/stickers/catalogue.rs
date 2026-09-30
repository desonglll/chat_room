//! Reading stickers out of the database in the shapes the API returns.
//!
//! Every list (a set, the installed library, recents, favorites, emoji search) is the same
//! sticker projection filtered and ordered differently, so they share one loader.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use sqlx::{FromRow, QueryBuilder};
use uuid::Uuid;

use super::models::{SetType, Sticker, StickerFormat, StickerSet};
use crate::state::{with_pool, AppState};

pub(crate) const RECENT_LIMIT: i64 = 20;
pub(crate) const FAVORITE_LIMIT: i64 = 5;
pub(crate) const SEARCH_LIMIT: i64 = 50;

const STICKER_COLUMNS: &str = "SELECT stickers.id, stickers.set_id, stickers.emoji, \
    stickers.format, stickers.mime_type, stickers.width, stickers.height, \
    stickers.duration_ms, stickers.size_bytes, stickers.access_key FROM stickers ";

/// Which stickers to load, and in what order. Removed stickers are never listed.
#[derive(Debug, Clone)]
pub(crate) enum StickerScope {
    Set(Uuid),
    InstalledBy(Uuid),
    RecentOf(Uuid),
    FavoritesOf(Uuid),
    Search { user_id: Uuid, emoji: String },
}

#[derive(FromRow)]
struct StickerRow {
    id: Uuid,
    set_id: Uuid,
    emoji: String,
    format: String,
    mime_type: String,
    width: i64,
    height: i64,
    duration_ms: Option<i64>,
    size_bytes: i64,
    access_key: Uuid,
}

#[derive(FromRow)]
pub(crate) struct SetRow {
    pub id: Uuid,
    pub short_name: String,
    pub title: String,
    pub set_type: String,
    pub owner_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub(crate) const SET_COLUMNS: &str = "SELECT sticker_sets.id, sticker_sets.short_name, \
    sticker_sets.title, sticker_sets.set_type, sticker_sets.owner_id, sticker_sets.created_at, \
    sticker_sets.updated_at FROM sticker_sets ";

impl SetRow {
    pub(crate) fn set_type(&self) -> SetType {
        SetType::parse(&self.set_type).unwrap_or_default()
    }

    pub(crate) fn into_set(self, stickers: Vec<Sticker>, library: Option<bool>) -> StickerSet {
        StickerSet {
            set_type: self.set_type(),
            id: self.id,
            short_name: self.short_name,
            title: self.title,
            owner_id: self.owner_id,
            stickers,
            installed: library.is_some(),
            archived: library.unwrap_or(false),
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

/// The catalogue capability URL of one sticker file.
pub(crate) fn sticker_file_url(id: Uuid, access_key: Uuid) -> String {
    format!("/api/stickers/{id}/file?key={access_key}")
}

impl AppState {
    pub(crate) async fn load_stickers(
        &self,
        scope: &StickerScope,
    ) -> Result<Vec<Sticker>, sqlx::Error> {
        let (filter, key, emoji) = match scope {
            StickerScope::Set(id) => (
                "WHERE stickers.set_id = $1 AND stickers.removed_at IS NULL \
                 ORDER BY stickers.position"
                    .to_string(),
                *id,
                None,
            ),
            StickerScope::InstalledBy(user) => (
                "JOIN user_sticker_sets lib ON lib.set_id = stickers.set_id \
                 WHERE lib.user_id = $1 AND stickers.removed_at IS NULL \
                 ORDER BY lib.position, stickers.position"
                    .to_string(),
                *user,
                None,
            ),
            StickerScope::RecentOf(user) => (
                format!(
                    "JOIN user_recent_stickers recent ON recent.sticker_id = stickers.id \
                     WHERE recent.user_id = $1 AND stickers.removed_at IS NULL \
                     ORDER BY recent.used_at DESC LIMIT {RECENT_LIMIT}"
                ),
                *user,
                None,
            ),
            StickerScope::FavoritesOf(user) => (
                format!(
                    "JOIN user_favorite_stickers fav ON fav.sticker_id = stickers.id \
                     WHERE fav.user_id = $1 AND stickers.removed_at IS NULL \
                     ORDER BY fav.created_at DESC LIMIT {FAVORITE_LIMIT}"
                ),
                *user,
                None,
            ),
            StickerScope::Search { user_id, emoji } => (
                format!(
                    "JOIN user_sticker_sets lib ON lib.set_id = stickers.set_id \
                     JOIN sticker_emojis hit ON hit.sticker_id = stickers.id \
                     WHERE lib.user_id = $1 AND lib.archived_at IS NULL \
                     AND hit.emoji = $2 AND stickers.removed_at IS NULL \
                     ORDER BY lib.position, stickers.position LIMIT {SEARCH_LIMIT}"
                ),
                *user_id,
                Some(emoji.clone()),
            ),
        };
        let query = format!("{STICKER_COLUMNS}{filter}");
        let rows: Vec<StickerRow> = with_pool!(self, |pool| {
            let mut statement = sqlx::query_as(&query).bind(key);
            if let Some(emoji) = &emoji {
                statement = statement.bind(emoji.clone());
            }
            statement.fetch_all(pool).await
        })?;
        let ids: Vec<Uuid> = rows.iter().map(|row| row.id).collect();
        let mut emojis = self.sticker_emojis(&ids).await?;
        Ok(rows
            .into_iter()
            .map(|row| {
                let list = emojis.remove(&row.id).unwrap_or_default();
                row.into_sticker(list)
            })
            .collect())
    }

    async fn sticker_emojis(
        &self,
        ids: &[Uuid],
    ) -> Result<HashMap<Uuid, Vec<String>>, sqlx::Error> {
        let mut grouped: HashMap<Uuid, Vec<String>> = HashMap::new();
        if ids.is_empty() {
            return Ok(grouped);
        }
        let rows: Vec<(Uuid, String)> = with_pool!(self, |pool| {
            let mut query = QueryBuilder::new(
                "SELECT sticker_id, emoji FROM sticker_emojis WHERE sticker_id IN (",
            );
            {
                let mut values = query.separated(", ");
                for id in ids {
                    values.push_bind(*id);
                }
            }
            query.push(") ORDER BY sticker_id, position");
            query.build_query_as().fetch_all(pool).await
        })?;
        for (sticker_id, emoji) in rows {
            grouped.entry(sticker_id).or_default().push(emoji);
        }
        Ok(grouped)
    }

    pub(crate) async fn sticker_set_row_by_short_name(
        &self,
        short_name: &str,
    ) -> Result<Option<SetRow>, sqlx::Error> {
        let query = format!("{SET_COLUMNS}WHERE sticker_sets.short_name = $1");
        let short_name = short_name.to_ascii_lowercase();
        with_pool!(self, |pool| {
            sqlx::query_as(&query)
                .bind(&short_name)
                .fetch_optional(pool)
                .await
        })
    }

    /// `None` = not installed; `Some(archived)` otherwise.
    pub(crate) async fn sticker_library_state(
        &self,
        user_id: Uuid,
        set_id: Uuid,
    ) -> Result<Option<bool>, sqlx::Error> {
        let archived_at: Option<Option<DateTime<Utc>>> = with_pool!(self, |pool| {
            sqlx::query_scalar(
                "SELECT archived_at FROM user_sticker_sets WHERE user_id = $1 AND set_id = $2",
            )
            .bind(user_id)
            .bind(set_id)
            .fetch_optional(pool)
            .await
        })?;
        Ok(archived_at.map(|archived_at| archived_at.is_some()))
    }

    /// A set as `viewer_id` sees it: live stickers plus the viewer's library state.
    pub async fn sticker_set_by_short_name(
        &self,
        short_name: &str,
        viewer_id: Uuid,
    ) -> Result<Option<StickerSet>, sqlx::Error> {
        let Some(row) = self.sticker_set_row_by_short_name(short_name).await? else {
            return Ok(None);
        };
        let stickers = self.load_stickers(&StickerScope::Set(row.id)).await?;
        let library = self.sticker_library_state(viewer_id, row.id).await?;
        Ok(Some(row.into_set(stickers, library)))
    }
}

impl StickerRow {
    fn into_sticker(self, emojis: Vec<String>) -> Sticker {
        let emojis = if emojis.is_empty() {
            vec![self.emoji.clone()]
        } else {
            emojis
        };
        Sticker {
            file_url: sticker_file_url(self.id, self.access_key),
            id: self.id,
            set_id: self.set_id,
            emoji: self.emoji,
            emojis,
            format: StickerFormat::parse(&self.format).unwrap_or(StickerFormat::Webp),
            mime_type: self.mime_type,
            width: self.width,
            height: self.height,
            duration_ms: self.duration_ms,
            size_bytes: self.size_bytes,
        }
    }
}
