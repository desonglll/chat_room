//! Sending a GIF into a chat.
//!
//! A GIF message is an attachment message with `media_kind = 'gif'`: a new `attachments`
//! row (with its own capability key) pointing at a content-addressed object, inserted in
//! the same transaction as the message, under the same in-transaction `message.send`
//! re-check as `finalize_attachment_message` — so no reader observes it half-classified and
//! history, realtime delivery, recall and the orphan sweep treat it like any attachment.
//!
//! The file comes from one of three sources: one of the sender's saved GIFs, a GIF message
//! the sender can read (the "recent GIFs from my chats" list), or a freshly uploaded
//! animation validated by [`super::animation`].

use chrono::Utc;
use uuid::Uuid;

use super::animation::{validate_animation, Animation};
use super::library::GifFile;
use super::models::MEDIA_KIND_GIF;
use crate::models::{StoredMessage, User};
use crate::state::{with_pool, AppState};
use crate::stickers::errors::StickerError;

pub enum GifSource {
    Saved(Uuid),
    Message(Uuid),
    Upload(Vec<u8>),
}

/// What a send did: a replayed `client_message_id` returns the original message.
pub struct GifSend {
    pub message: StoredMessage,
    pub inserted: bool,
}

fn file_name(mime_type: &str) -> &'static str {
    match mime_type {
        "video/mp4" => "gif.mp4",
        "video/webm" => "gif.webm",
        _ => "gif.gif",
    }
}

impl AppState {
    pub async fn send_gif_message(
        &self,
        room_id: Uuid,
        sender: &User,
        sender_display_name: &str,
        source: GifSource,
        reply_to: Option<Uuid>,
        client_message_id: Option<Uuid>,
    ) -> Result<GifSend, StickerError> {
        if let Some(existing) = self
            .replayed_gif_send(room_id, sender.id, client_message_id)
            .await?
        {
            return Ok(existing);
        }
        let saved_id = match &source {
            GifSource::Saved(id) => Some(*id),
            _ => None,
        };
        let file = match source {
            GifSource::Saved(id) => self
                .saved_gif_file(sender.id, id)
                .await?
                .ok_or(StickerError::NotFound("saved_gif_not_found"))?,
            GifSource::Message(id) => self.readable_gif_file(sender.id, id).await?,
            GifSource::Upload(bytes) => {
                let animation = validate_animation(&bytes)
                    .map_err(|rejection| StickerError::Invalid(rejection.code()))?;
                self.store_animation(&bytes, animation).await?
            }
        };
        if !self
            .attachment_store()
            .exists(&file.storage_key)
            .await
            .map_err(StickerError::Storage)?
        {
            return Err(StickerError::NotFound("gif_unavailable"));
        }
        let message_id = self
            .insert_gif_message(
                room_id,
                sender,
                sender_display_name,
                &file,
                reply_to,
                client_message_id,
            )
            .await;
        let message_id = match message_id {
            Ok(id) => id,
            Err(sqlx::Error::RowNotFound) => return Err(StickerError::Forbidden),
            Err(sqlx::Error::Database(error)) if error.is_unique_violation() => {
                // A concurrent retry with the same client_message_id won the insert.
                return self
                    .replayed_gif_send(room_id, sender.id, client_message_id)
                    .await?
                    .ok_or(StickerError::Conflict("client_message_id_in_use"));
            }
            Err(error) => return Err(error.into()),
        };
        self.invalidate_message_cache(room_id).await;
        if let Some(id) = saved_id {
            if let Err(error) = self.touch_saved_gif(sender.id, id).await {
                tracing::warn!("reorder saved gif failed: {error}");
            }
        }
        let message = self
            .message_by_id(message_id, Some(sender.id))
            .await?
            .ok_or(StickerError::NotFound("message_not_found"))?;
        Ok(GifSend {
            message,
            inserted: true,
        })
    }

    async fn insert_gif_message(
        &self,
        room_id: Uuid,
        sender: &User,
        sender_display_name: &str,
        file: &GifFile,
        reply_to: Option<Uuid>,
        client_message_id: Option<Uuid>,
    ) -> Result<Uuid, sqlx::Error> {
        let reply_to = self.reply_preview(room_id, reply_to).await?;
        let attachment_id = Uuid::new_v4();
        let message_id = Uuid::new_v4();
        let created_at = Utc::now();
        with_pool!(self, |pool| {
            async {
                let mut tx = pool.begin().await?;
                let allowed: bool = sqlx::query_scalar(
                    "SELECT EXISTS(SELECT 1 FROM chat_members \
                     JOIN chat_role_permissions \
                       ON chat_role_permissions.role_id = chat_members.role_id \
                     WHERE chat_members.room_id = $1 AND chat_members.user_id = $2 \
                       AND chat_members.status = 'active' \
                       AND chat_role_permissions.permission_key = 'message.send')",
                )
                .bind(room_id)
                .bind(sender.id)
                .fetch_one(&mut *tx)
                .await?;
                if !allowed {
                    return Err(sqlx::Error::RowNotFound);
                }
                sqlx::query(
                    "INSERT INTO attachments \
                     (id, access_key, room_id, uploader_id, file_name, mime_type, size_bytes, \
                      is_sensitive, created_at, content_hash, storage_key) \
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)",
                )
                .bind(attachment_id)
                .bind(Uuid::new_v4())
                .bind(room_id)
                .bind(sender.id)
                .bind(file_name(&file.mime_type))
                .bind(&file.mime_type)
                .bind(file.size_bytes)
                .bind(false)
                .bind(created_at)
                .bind(&file.content_hash)
                .bind(&file.storage_key)
                .execute(&mut *tx)
                .await?;
                sqlx::query(
                    "INSERT INTO messages (id, room_id, sender_id, sender, content, \
                     attachment_id, reply_to_id, client_message_id, media_kind, created_at) \
                     VALUES ($1, $2, $3, $4, '', $5, $6, $7, $8, $9)",
                )
                .bind(message_id)
                .bind(room_id)
                .bind(sender.id)
                .bind(sender_display_name)
                .bind(attachment_id)
                .bind(reply_to.as_ref().map(|reply| reply.message_id))
                .bind(client_message_id)
                .bind(MEDIA_KIND_GIF)
                .bind(created_at)
                .execute(&mut *tx)
                .await?;
                sqlx::query(
                    "UPDATE attachments SET orphaned_at = NULL \
                     WHERE storage_key = $1 AND orphaned_at IS NOT NULL",
                )
                .bind(&file.storage_key)
                .execute(&mut *tx)
                .await?;
                tx.commit().await
            }
            .await
        })?;
        Ok(message_id)
    }

    /// Commit uploaded bytes under their SHA-256 (identical bytes are stored once) and
    /// record the geometry read from the header.
    async fn store_animation(
        &self,
        bytes: &[u8],
        animation: Animation,
    ) -> Result<GifFile, StickerError> {
        let store = self.attachment_store();
        let mut staged = store.begin().await.map_err(StickerError::Storage)?;
        staged.write(bytes).await.map_err(StickerError::Storage)?;
        let content_hash = store
            .hash_staged(&mut staged)
            .await
            .map_err(StickerError::Storage)?;
        let size_bytes = staged.size();
        let _guard = self.content_hash_locks().lock(&content_hash).await;
        if store
            .exists(&content_hash)
            .await
            .map_err(StickerError::Storage)?
        {
            drop(staged);
        } else {
            store
                .commit(staged, &content_hash)
                .await
                .map_err(StickerError::Storage)?;
        }
        with_pool!(self, |pool| {
            sqlx::query(
                "INSERT INTO animation_metadata (content_hash, width, height, duration_ms, \
                 created_at) VALUES ($1, $2, $3, $4, $5) ON CONFLICT (content_hash) DO NOTHING",
            )
            .bind(&content_hash)
            .bind(i64::from(animation.width))
            .bind(i64::from(animation.height))
            .bind(animation.duration_ms.map(i64::from))
            .bind(Utc::now())
            .execute(pool)
            .await
            .map(|_| ())
        })?;
        Ok(GifFile {
            storage_key: content_hash.clone(),
            content_hash,
            mime_type: animation.format.mime_type().to_string(),
            size_bytes,
        })
    }

    async fn replayed_gif_send(
        &self,
        room_id: Uuid,
        sender_id: Uuid,
        client_message_id: Option<Uuid>,
    ) -> Result<Option<GifSend>, sqlx::Error> {
        let Some(client_message_id) = client_message_id else {
            return Ok(None);
        };
        let existing: Option<Uuid> = with_pool!(self, |pool| {
            sqlx::query_scalar(
                "SELECT id FROM messages WHERE room_id = $1 AND sender_id = $2 \
                 AND client_message_id = $3",
            )
            .bind(room_id)
            .bind(sender_id)
            .bind(client_message_id)
            .fetch_optional(pool)
            .await
        })?;
        let Some(id) = existing else {
            return Ok(None);
        };
        Ok(self
            .message_by_id(id, Some(sender_id))
            .await?
            .map(|message| GifSend {
                message,
                inserted: false,
            }))
    }
}
