//! Each account's saved GIFs, and the GIFs recently sent into chats it can read.
//!
//! Authorization mirrors attachments and favorites: a GIF can be saved only from a live
//! (not recalled) message in a live chat where the account is an active member — the same
//! membership that lets it read that chat's history and so learn the attachment's
//! capability URL. Once saved, the GIF is the account's own reference to the content
//! object (like a favorite) and outlives the source message.

use chrono::{DateTime, Utc};
use sqlx::FromRow;
use uuid::Uuid;

use super::models::{RecentGif, SavedGif, MEDIA_KIND_GIF, RECENT_GIF_LIMIT, SAVED_GIF_LIMIT};
use crate::state::{with_pool, AppState};
use crate::stickers::errors::StickerError;

/// The content object a GIF message or saved GIF points at.
#[derive(Debug, Clone)]
pub(crate) struct GifFile {
    pub content_hash: String,
    pub storage_key: String,
    pub mime_type: String,
    pub size_bytes: i64,
}

/// A message is a GIF when it was sent as one, or when its attachment is a real GIF file
/// (sent through the ordinary attachment path, or by a client that predates TG-305).
pub(crate) fn is_gif(media_kind: Option<&str>, mime_type: &str) -> bool {
    media_kind == Some(MEDIA_KIND_GIF) || mime_type.eq_ignore_ascii_case("image/gif")
}

#[derive(FromRow)]
struct SavedRow {
    id: Uuid,
    mime_type: String,
    size_bytes: i64,
    access_key: Uuid,
    width: Option<i64>,
    height: Option<i64>,
    duration_ms: Option<i64>,
    saved_at: DateTime<Utc>,
    used_at: DateTime<Utc>,
}

impl From<SavedRow> for SavedGif {
    fn from(row: SavedRow) -> Self {
        SavedGif {
            id: row.id,
            mime_type: row.mime_type,
            size_bytes: row.size_bytes,
            width: row.width,
            height: row.height,
            duration_ms: row.duration_ms,
            file_url: format!("/api/gifs/saved/{}/file?key={}", row.id, row.access_key),
            saved_at: row.saved_at,
            used_at: row.used_at,
        }
    }
}

const SAVED_SELECT: &str = "SELECT g.id, g.mime_type, g.size_bytes, g.access_key, \
     md.width, md.height, md.duration_ms, g.saved_at, g.used_at FROM user_saved_gifs g \
     LEFT JOIN animation_metadata md ON md.content_hash = g.content_hash";

#[derive(FromRow)]
struct RecentRow {
    message_id: Uuid,
    room_id: Uuid,
    attachment_id: Uuid,
    access_key: Uuid,
    mime_type: String,
    size_bytes: i64,
    content_hash: Option<String>,
    width: Option<i64>,
    height: Option<i64>,
    duration_ms: Option<i64>,
    created_at: DateTime<Utc>,
}

type ReadableRow = (Option<String>, Option<String>, String, i64, Option<String>);

impl AppState {
    /// The GIF file of `message_id` if `user_id` can read that message right now.
    /// Unreadable and nonexistent answer the same `NotFound`, so existence never leaks.
    pub(crate) async fn readable_gif_file(
        &self,
        user_id: Uuid,
        message_id: Uuid,
    ) -> Result<GifFile, StickerError> {
        let row: Option<ReadableRow> = with_pool!(self, |pool| {
            sqlx::query_as(
                "SELECT a.content_hash, a.storage_key, a.mime_type, a.size_bytes, m.media_kind \
                 FROM messages m JOIN attachments a ON a.id = m.attachment_id \
                 JOIN chats c ON c.id = m.room_id \
                 WHERE m.id = $1 AND m.recalled_at IS NULL AND c.deleted_at IS NULL \
                   AND EXISTS (SELECT 1 FROM chat_members cm WHERE cm.room_id = m.room_id \
                     AND cm.user_id = $2 AND cm.status = 'active')",
            )
            .bind(message_id)
            .bind(user_id)
            .fetch_optional(pool)
            .await
        })?;
        let (content_hash, storage_key, mime_type, size_bytes, media_kind) =
            row.ok_or(StickerError::NotFound("message_not_found"))?;
        if !is_gif(media_kind.as_deref(), &mime_type) {
            return Err(StickerError::Invalid("not_a_gif"));
        }
        // Legacy (pre content-addressing) objects cannot be shared by reference.
        let (Some(content_hash), Some(storage_key)) = (content_hash, storage_key) else {
            return Err(StickerError::Invalid("gif_unavailable"));
        };
        Ok(GifFile {
            content_hash,
            storage_key,
            mime_type,
            size_bytes,
        })
    }

    /// Save (or move to the front) the GIF of a readable message. `true` when it was new.
    pub async fn save_gif(
        &self,
        user_id: Uuid,
        message_id: Uuid,
    ) -> Result<(SavedGif, bool), StickerError> {
        let file = self.readable_gif_file(user_id, message_id).await?;
        let now = Utc::now();
        let existed: Option<Uuid> = with_pool!(self, |pool| {
            sqlx::query_scalar(
                "SELECT id FROM user_saved_gifs WHERE user_id = $1 AND content_hash = $2",
            )
            .bind(user_id)
            .bind(&file.content_hash)
            .fetch_optional(pool)
            .await
        })?;
        let evicted: Vec<String> = with_pool!(self, |pool| {
            async {
                let mut tx = pool.begin().await?;
                sqlx::query(
                    "INSERT INTO user_saved_gifs (id, user_id, content_hash, storage_key, \
                     mime_type, size_bytes, access_key, source_message_id, saved_at, used_at) \
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $9) \
                     ON CONFLICT (user_id, content_hash) DO UPDATE SET used_at = excluded.used_at",
                )
                .bind(Uuid::new_v4())
                .bind(user_id)
                .bind(&file.content_hash)
                .bind(&file.storage_key)
                .bind(&file.mime_type)
                .bind(file.size_bytes)
                .bind(Uuid::new_v4())
                .bind(message_id)
                .bind(now)
                .execute(&mut *tx)
                .await?;
                sqlx::query(
                    "UPDATE attachments SET orphaned_at = NULL \
                     WHERE storage_key = $1 AND orphaned_at IS NOT NULL",
                )
                .bind(&file.storage_key)
                .execute(&mut *tx)
                .await?;
                let evicted: Vec<String> = sqlx::query_scalar(&format!(
                    "DELETE FROM user_saved_gifs WHERE user_id = $1 AND id NOT IN (\
                       SELECT id FROM user_saved_gifs WHERE user_id = $1 \
                       ORDER BY used_at DESC, id DESC LIMIT {SAVED_GIF_LIMIT}) \
                     RETURNING storage_key"
                ))
                .bind(user_id)
                .fetch_all(&mut *tx)
                .await?;
                tx.commit().await?;
                Ok::<_, sqlx::Error>(evicted)
            }
            .await
        })?;
        for storage_key in evicted {
            self.release_gif_storage(&storage_key).await?;
        }
        let saved = self
            .saved_gifs_where(user_id, Some(&file.content_hash))
            .await?
            .pop()
            .ok_or(StickerError::NotFound("saved_gif_not_found"))?;
        Ok((saved, existed.is_none()))
    }

    /// Most recently used first.
    pub async fn saved_gifs(&self, user_id: Uuid) -> Result<Vec<SavedGif>, sqlx::Error> {
        self.saved_gifs_where(user_id, None).await
    }

    async fn saved_gifs_where(
        &self,
        user_id: Uuid,
        content_hash: Option<&str>,
    ) -> Result<Vec<SavedGif>, sqlx::Error> {
        let filter = if content_hash.is_some() {
            " AND g.content_hash = $2"
        } else {
            ""
        };
        let query = format!(
            "{SAVED_SELECT} WHERE g.user_id = $1{filter} ORDER BY g.used_at DESC, g.id DESC"
        );
        let rows: Vec<SavedRow> = with_pool!(self, |pool| {
            let mut statement = sqlx::query_as(&query).bind(user_id);
            if let Some(hash) = content_hash {
                statement = statement.bind(hash);
            }
            statement.fetch_all(pool).await
        })?;
        Ok(rows.into_iter().map(SavedGif::from).collect())
    }

    /// `false` when the account has no such saved GIF.
    pub async fn remove_saved_gif(&self, user_id: Uuid, id: Uuid) -> Result<bool, StickerError> {
        let storage_key: Option<String> = with_pool!(self, |pool| {
            sqlx::query_scalar(
                "DELETE FROM user_saved_gifs WHERE id = $1 AND user_id = $2 RETURNING storage_key",
            )
            .bind(id)
            .bind(user_id)
            .fetch_optional(pool)
            .await
        })?;
        let Some(storage_key) = storage_key else {
            return Ok(false);
        };
        self.release_gif_storage(&storage_key).await?;
        Ok(true)
    }

    /// The file behind one of the account's saved GIFs.
    pub(crate) async fn saved_gif_file(
        &self,
        user_id: Uuid,
        id: Uuid,
    ) -> Result<Option<GifFile>, sqlx::Error> {
        let row: Option<(String, String, String, i64)> = with_pool!(self, |pool| {
            sqlx::query_as(
                "SELECT content_hash, storage_key, mime_type, size_bytes FROM user_saved_gifs \
                 WHERE id = $1 AND user_id = $2",
            )
            .bind(id)
            .bind(user_id)
            .fetch_optional(pool)
            .await
        })?;
        Ok(row.map(
            |(content_hash, storage_key, mime_type, size_bytes)| GifFile {
                content_hash,
                storage_key,
                mime_type,
                size_bytes,
            },
        ))
    }

    /// Sending a saved GIF moves it to the front, as in Telegram.
    pub(crate) async fn touch_saved_gif(&self, user_id: Uuid, id: Uuid) -> Result<(), sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query("UPDATE user_saved_gifs SET used_at = $1 WHERE id = $2 AND user_id = $3")
                .bind(Utc::now())
                .bind(id)
                .bind(user_id)
                .execute(pool)
                .await
                .map(|_| ())
        })
    }

    /// A saved GIF no longer pins its object: let the orphan sweep decide again.
    async fn release_gif_storage(&self, storage_key: &str) -> Result<(), sqlx::Error> {
        let attachment: Option<Uuid> = with_pool!(self, |pool| {
            sqlx::query_scalar("SELECT id FROM attachments WHERE storage_key = $1 LIMIT 1")
                .bind(storage_key)
                .fetch_optional(pool)
                .await
        })?;
        match attachment {
            Some(id) => self.recompute_attachment_orphan_status(id).await,
            None => Ok(()),
        }
    }

    /// GIFs recently sent into chats the account reads, newest first, one per file.
    ///
    /// Chats with a password are left out: their history needs the password on every read,
    /// and this list would otherwise reveal their capability URLs without it. Sensitive
    /// attachments are left out because the panel has no veil.
    pub async fn recent_gifs(
        &self,
        user_id: Uuid,
        limit: Option<i64>,
    ) -> Result<Vec<RecentGif>, sqlx::Error> {
        let limit = limit.unwrap_or(RECENT_GIF_LIMIT).clamp(1, RECENT_GIF_LIMIT);
        let rows: Vec<RecentRow> = with_pool!(self, |pool| {
            sqlx::query_as(
                "SELECT m.id AS message_id, m.room_id, a.id AS attachment_id, a.access_key, \
                   a.mime_type, a.size_bytes, a.content_hash, md.width, md.height, \
                   md.duration_ms, m.created_at \
                 FROM messages m JOIN attachments a ON a.id = m.attachment_id \
                 JOIN chats c ON c.id = m.room_id \
                 JOIN chat_members cm ON cm.room_id = m.room_id AND cm.user_id = $1 \
                   AND cm.status = 'active' \
                 LEFT JOIN animation_metadata md ON md.content_hash = a.content_hash \
                 WHERE m.recalled_at IS NULL AND c.deleted_at IS NULL \
                   AND COALESCE(c.password_hash, '') = '' AND a.is_sensitive = $2 \
                   AND (m.media_kind = $3 OR a.mime_type = 'image/gif') \
                 ORDER BY m.created_at DESC, m.id DESC LIMIT $4",
            )
            .bind(user_id)
            .bind(false)
            .bind(MEDIA_KIND_GIF)
            .bind(limit * 3)
            .fetch_all(pool)
            .await
        })?;
        let mut seen = std::collections::HashSet::new();
        Ok(rows
            .into_iter()
            .filter(|row| {
                let identity = row
                    .content_hash
                    .clone()
                    .unwrap_or_else(|| row.attachment_id.to_string());
                seen.insert(identity)
            })
            .take(limit as usize)
            .map(|row| RecentGif {
                message_id: row.message_id,
                room_id: row.room_id,
                mime_type: row.mime_type,
                size_bytes: row.size_bytes,
                width: row.width,
                height: row.height,
                duration_ms: row.duration_ms,
                file_url: format!(
                    "/api/attachments/{}?key={}",
                    row.attachment_id, row.access_key
                ),
                created_at: row.created_at,
            })
            .collect())
    }
}
