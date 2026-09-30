//! Sending a sticker into a chat.
//!
//! A sticker message is an attachment message: the transaction below is the attachment
//! finaliser (`finalize_attachment_message`) with `media_kind = 'sticker'` and the sticker
//! reference added in the same insert, so no reader can observe the message half-classified.
//! The new `attachments` row points at the sticker's content-addressed object and carries
//! its own access key, so the file is authorized exactly like any other attachment: only
//! the chat's history ever reveals that capability URL.

use chrono::Utc;
use uuid::Uuid;

use super::errors::StickerError;
use super::models::{SendStickerRequest, MEDIA_KIND_STICKER};
use crate::models::{StoredMessage, User};
use crate::state::{with_pool, AppState};

/// What a send did: a replayed `client_message_id` returns the original message.
pub struct StickerSend {
    pub message: StoredMessage,
    pub inserted: bool,
}

type StickerFileRow = (String, String, i64, String, String);

impl AppState {
    pub async fn send_sticker_message(
        &self,
        room_id: Uuid,
        sender: &User,
        sender_display_name: &str,
        request: &SendStickerRequest,
    ) -> Result<StickerSend, StickerError> {
        if let Some(existing) = self
            .replayed_sticker_send(room_id, sender.id, request.client_message_id)
            .await?
        {
            return Ok(existing);
        }
        let sticker: Option<StickerFileRow> = with_pool!(self, |pool| {
            sqlx::query_as(
                "SELECT format, mime_type, size_bytes, content_hash, storage_key FROM stickers \
                 WHERE id = $1 AND removed_at IS NULL",
            )
            .bind(request.sticker_id)
            .fetch_optional(pool)
            .await
        })?;
        let (format, mime_type, size_bytes, content_hash, storage_key) =
            sticker.ok_or(StickerError::NotFound("sticker_not_found"))?;
        let reply_to = self.reply_preview(room_id, request.reply_to).await?;
        let attachment_id = Uuid::new_v4();
        let message_id = Uuid::new_v4();
        let created_at = Utc::now();
        let file_name = format!("sticker.{format}");
        let persisted: Result<(), sqlx::Error> = with_pool!(self, |pool| {
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
                .bind(&file_name)
                .bind(&mime_type)
                .bind(size_bytes)
                .bind(false)
                .bind(created_at)
                .bind(&content_hash)
                .bind(&storage_key)
                .execute(&mut *tx)
                .await?;
                sqlx::query(
                    "INSERT INTO messages (id, room_id, sender_id, sender, content, \
                     attachment_id, reply_to_id, client_message_id, media_kind, sticker_id, \
                     created_at, topic_id) VALUES ($1, $2, $3, $4, '', $5, $6, $7, $8, $9, $10, $11)",
                )
                .bind(message_id)
                .bind(room_id)
                .bind(sender.id)
                .bind(sender_display_name)
                .bind(attachment_id)
                .bind(reply_to.as_ref().map(|reply| reply.message_id))
                .bind(request.client_message_id)
                .bind(MEDIA_KIND_STICKER)
                .bind(request.sticker_id)
                .bind(created_at)
                .bind(request.topic_id)
                .execute(&mut *tx)
                .await?;
                sqlx::query(
                    "UPDATE attachments SET orphaned_at = NULL \
                     WHERE storage_key = $1 AND orphaned_at IS NOT NULL",
                )
                .bind(&storage_key)
                .execute(&mut *tx)
                .await?;
                tx.commit().await
            }
            .await
        });
        match persisted {
            Ok(()) => {}
            Err(sqlx::Error::RowNotFound) => return Err(StickerError::Forbidden),
            Err(sqlx::Error::Database(error)) if error.is_unique_violation() => {
                // A concurrent retry with the same client_message_id won the insert.
                return self
                    .replayed_sticker_send(room_id, sender.id, request.client_message_id)
                    .await?
                    .ok_or(StickerError::Conflict("client_message_id_in_use"));
            }
            Err(error) => return Err(error.into()),
        }
        self.invalidate_message_cache(room_id).await;
        if let Err(error) = self
            .record_recent_sticker(sender.id, request.sticker_id)
            .await
        {
            tracing::warn!("record recent sticker failed: {error}");
        }
        let message = self
            .message_by_id(message_id, Some(sender.id))
            .await?
            .ok_or(StickerError::NotFound("message_not_found"))?;
        Ok(StickerSend {
            message,
            inserted: true,
        })
    }

    async fn replayed_sticker_send(
        &self,
        room_id: Uuid,
        sender_id: Uuid,
        client_message_id: Option<Uuid>,
    ) -> Result<Option<StickerSend>, sqlx::Error> {
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
            .map(|message| StickerSend {
                message,
                inserted: false,
            }))
    }
}
