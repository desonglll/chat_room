//! Recently sent and favorite stickers, and emoji search over the installed library.

use chrono::Utc;
use uuid::Uuid;

use super::catalogue::{StickerScope, FAVORITE_LIMIT, RECENT_LIMIT};
use super::errors::StickerError;
use super::models::Sticker;
use crate::state::{with_pool, AppState};

impl AppState {
    /// Most recent first, at most [`RECENT_LIMIT`].
    pub async fn recent_stickers(&self, user_id: Uuid) -> Result<Vec<Sticker>, sqlx::Error> {
        self.load_stickers(&StickerScope::RecentOf(user_id)).await
    }

    /// Newest first, at most [`FAVORITE_LIMIT`].
    pub async fn favorite_stickers(&self, user_id: Uuid) -> Result<Vec<Sticker>, sqlx::Error> {
        self.load_stickers(&StickerScope::FavoritesOf(user_id))
            .await
    }

    /// Stickers tagged with `emoji` in the caller's non-archived installed sets.
    pub async fn search_stickers(
        &self,
        user_id: Uuid,
        emoji: &str,
    ) -> Result<Vec<Sticker>, sqlx::Error> {
        self.load_stickers(&StickerScope::Search {
            user_id,
            emoji: emoji.trim().to_string(),
        })
        .await
    }

    /// Move `sticker_id` to the front of the account's recents and trim to the cap.
    pub(crate) async fn record_recent_sticker(
        &self,
        user_id: Uuid,
        sticker_id: Uuid,
    ) -> Result<(), sqlx::Error> {
        let now = Utc::now();
        with_pool!(self, |pool| {
            async {
                sqlx::query(
                    "INSERT INTO user_recent_stickers (user_id, sticker_id, used_at) \
                     VALUES ($1, $2, $3) \
                     ON CONFLICT (user_id, sticker_id) DO UPDATE SET used_at = excluded.used_at",
                )
                .bind(user_id)
                .bind(sticker_id)
                .bind(now)
                .execute(pool)
                .await?;
                sqlx::query(&trim_query("user_recent_stickers", "used_at", RECENT_LIMIT))
                    .bind(user_id)
                    .execute(pool)
                    .await?;
                Ok(())
            }
            .await
        })
    }

    pub async fn remove_recent_sticker(
        &self,
        user_id: Uuid,
        sticker_id: Uuid,
    ) -> Result<(), sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query("DELETE FROM user_recent_stickers WHERE user_id = $1 AND sticker_id = $2")
                .bind(user_id)
                .bind(sticker_id)
                .execute(pool)
                .await
                .map(|_| ())
        })
    }

    /// Idempotent. Beyond the cap the oldest favorite is evicted, as in Telegram.
    pub async fn favorite_sticker(
        &self,
        user_id: Uuid,
        sticker_id: Uuid,
    ) -> Result<(), StickerError> {
        if !self.live_sticker_exists(sticker_id).await? {
            return Err(StickerError::NotFound("sticker_not_found"));
        }
        let now = Utc::now();
        with_pool!(self, |pool| {
            async {
                sqlx::query(
                    "INSERT INTO user_favorite_stickers (user_id, sticker_id, created_at) \
                     VALUES ($1, $2, $3) ON CONFLICT (user_id, sticker_id) DO NOTHING",
                )
                .bind(user_id)
                .bind(sticker_id)
                .bind(now)
                .execute(pool)
                .await?;
                sqlx::query(&trim_query(
                    "user_favorite_stickers",
                    "created_at",
                    FAVORITE_LIMIT,
                ))
                .bind(user_id)
                .execute(pool)
                .await?;
                Ok::<_, sqlx::Error>(())
            }
            .await
        })?;
        Ok(())
    }

    pub async fn unfavorite_sticker(
        &self,
        user_id: Uuid,
        sticker_id: Uuid,
    ) -> Result<(), sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query("DELETE FROM user_favorite_stickers WHERE user_id = $1 AND sticker_id = $2")
                .bind(user_id)
                .bind(sticker_id)
                .execute(pool)
                .await
                .map(|_| ())
        })
    }

    pub(crate) async fn live_sticker_exists(&self, sticker_id: Uuid) -> Result<bool, sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM stickers WHERE id = $1 AND removed_at IS NULL)",
            )
            .bind(sticker_id)
            .fetch_one(pool)
            .await
        })
    }
}

/// Keep the newest `limit` rows of one account in a per-account sticker list.
fn trim_query(table: &str, column: &str, limit: i64) -> String {
    format!(
        "DELETE FROM {table} WHERE user_id = $1 AND sticker_id NOT IN (\
         SELECT sticker_id FROM {table} WHERE user_id = $1 \
         ORDER BY {column} DESC, sticker_id LIMIT {limit})"
    )
}
