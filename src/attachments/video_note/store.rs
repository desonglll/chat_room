//! Video note persistence: the send transaction, the watched marks, and the two post-load
//! steps that add `StoredMessage::video_note` (static, cacheable) and its per-viewer
//! `listened` flag. Mirrors `attachments::voice::store` table for table.

use std::collections::{HashMap, HashSet};

use chrono::Utc;
use sqlx::QueryBuilder;
use uuid::Uuid;

use super::model::{encode_thumbnail, VideoNote, VideoNoteError, MEDIA_KIND_VIDEO_NOTE};
use crate::attachments::voice::model::{DurationSource, Listened};
use crate::models::{Attachment, StoredMessage, User};
use crate::state::{with_pool, AppState};

/// A validated video note, ready to be written.
pub(crate) struct NewVideoNote {
    pub file_name: String,
    pub mime_type: String,
    pub size_bytes: i64,
    pub content_hash: String,
    pub storage_key: String,
    pub duration_ms: u32,
    pub duration_source: DurationSource,
    pub thumbnail: Option<Vec<u8>>,
    pub reply_to: Option<Uuid>,
    pub topic_id: Option<Uuid>,
}

impl AppState {
    /// Write the attachment, the `media_kind = 'video_note'` message and its `video_notes` row
    /// in one transaction; the send permission is re-checked inside it.
    pub(crate) async fn insert_video_note_message(
        &self,
        room_id: Uuid,
        sender: &User,
        sender_display_name: &str,
        note: NewVideoNote,
    ) -> Result<StoredMessage, VideoNoteError> {
        let attachment_id = Uuid::new_v4();
        let access_key = Uuid::new_v4();
        let message_id = Uuid::new_v4();
        let created_at = Utc::now();
        let reply_to = self
            .reply_preview(room_id, note.reply_to)
            .await
            .map_err(VideoNoteError::from)?;
        let duration_ms = i32::try_from(note.duration_ms)
            .map_err(|_| VideoNoteError::Invalid("invalid_duration"))?;
        let persisted: Result<(), sqlx::Error> = with_pool!(self, |pool| {
            async {
                let mut tx = pool.begin().await?;
                let allowed: bool = sqlx::query_scalar(
                    "SELECT EXISTS(SELECT 1 FROM chat_members \
                     JOIN chat_role_permissions \
                       ON chat_role_permissions.role_id = chat_members.role_id \
                     WHERE chat_members.room_id = $1 AND chat_members.user_id = $2 \
                       AND chat_members.status = 'active' \
                       AND chat_role_permissions.permission_key IN ('message.send', 'message.post'))",
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
                .bind(access_key)
                .bind(room_id)
                .bind(sender.id)
                .bind(&note.file_name)
                .bind(&note.mime_type)
                .bind(note.size_bytes)
                .bind(false)
                .bind(created_at)
                .bind(&note.content_hash)
                .bind(&note.storage_key)
                .execute(&mut *tx)
                .await?;
                sqlx::query(
                    "INSERT INTO messages (id, room_id, sender_id, sender, content, \
                     attachment_id, reply_to_id, media_kind, created_at, topic_id) \
                     VALUES ($1, $2, $3, $4, '', $5, $6, $7, $8, $9)",
                )
                .bind(message_id)
                .bind(room_id)
                .bind(sender.id)
                .bind(sender_display_name)
                .bind(attachment_id)
                .bind(reply_to.as_ref().map(|reply| reply.message_id))
                .bind(MEDIA_KIND_VIDEO_NOTE)
                .bind(created_at)
                .bind(note.topic_id)
                .execute(&mut *tx)
                .await?;
                sqlx::query(
                    "INSERT INTO video_notes \
                     (message_id, duration_ms, thumbnail, duration_source, created_at) \
                     VALUES ($1, $2, $3, $4, $5)",
                )
                .bind(message_id)
                .bind(duration_ms)
                .bind(note.thumbnail.as_deref())
                .bind(note.duration_source.as_str())
                .bind(created_at)
                .execute(&mut *tx)
                .await?;
                sqlx::query(
                    "UPDATE attachments SET orphaned_at = NULL \
                     WHERE storage_key = $1 AND orphaned_at IS NOT NULL",
                )
                .bind(&note.storage_key)
                .execute(&mut *tx)
                .await?;
                tx.commit().await
            }
            .await
        });
        match persisted {
            Ok(()) => {}
            Err(sqlx::Error::RowNotFound) => return Err(VideoNoteError::Forbidden),
            Err(error) => {
                tracing::warn!(
                    "video note transaction failed; retained content object {}: {error}",
                    note.storage_key
                );
                return Err(error.into());
            }
        }
        Ok(StoredMessage {
            id: message_id,
            room_id,
            sender_id: Some(sender.id),
            sender: sender_display_name.to_string(),
            sender_avatar: sender.avatar_emoji.clone(),
            attachment: Some(Attachment {
                id: attachment_id,
                file_name: note.file_name,
                mime_type: note.mime_type,
                size_bytes: note.size_bytes,
                download_url: format!("/api/attachments/{attachment_id}?key={access_key}"),
                is_sensitive: false,
                thumbnail_url: None,
            }),
            reply_to,
            created_at,
            media_kind: Some(MEDIA_KIND_VIDEO_NOTE.to_string()),
            video_note: Some(VideoNote {
                duration_ms: note.duration_ms,
                thumbnail: encode_thumbnail(note.thumbnail),
                listened: false,
            }),
            ..Default::default()
        })
    }

    /// `(room_id, sender_id)` of a live video note in a live chat where `user_id` is an active
    /// member, else `NotFound` — one answer for all, so nothing leaks across chats.
    pub(crate) async fn video_note_listen_target(
        &self,
        message_id: Uuid,
        user_id: Uuid,
    ) -> Result<(Uuid, Option<Uuid>), VideoNoteError> {
        let row: Option<(Uuid, Option<Uuid>)> = with_pool!(self, |pool| {
            sqlx::query_as(
                "SELECT messages.room_id, messages.sender_id FROM video_notes \
                 JOIN messages ON messages.id = video_notes.message_id \
                   AND messages.recalled_at IS NULL \
                 JOIN chats ON chats.id = messages.room_id AND chats.deleted_at IS NULL \
                 WHERE video_notes.message_id = $1 AND EXISTS (SELECT 1 FROM chat_members \
                   WHERE chat_members.room_id = messages.room_id \
                     AND chat_members.user_id = $2 AND chat_members.status = 'active')",
            )
            .bind(message_id)
            .bind(user_id)
            .fetch_optional(pool)
            .await
        })?;
        row.ok_or(VideoNoteError::NotFound)
    }

    /// Record that `user_id` watched the message. `Some` only the first time.
    pub(crate) async fn record_video_note_listen(
        &self,
        message_id: Uuid,
        room_id: Uuid,
        sender_id: Option<Uuid>,
        user_id: Uuid,
    ) -> Result<Option<Listened>, VideoNoteError> {
        let inserted = with_pool!(self, |pool| {
            sqlx::query(
                "INSERT INTO video_note_listens (message_id, user_id, listened_at) \
                 VALUES ($1, $2, $3) ON CONFLICT (message_id, user_id) DO NOTHING",
            )
            .bind(message_id)
            .bind(user_id)
            .bind(Utc::now())
            .execute(pool)
            .await
            .map(|result| result.rows_affected())
        })?;
        Ok((inserted == 1).then_some(Listened {
            message_id,
            room_id,
            user_id,
            sender_id,
        }))
    }

    /// Static half: `video_note` (duration, thumbnail) for every video note whose attachment
    /// this viewer can see. Viewer-independent, so it runs before the Redis history cache.
    pub(crate) async fn attach_message_video_note(
        &self,
        messages: &mut [StoredMessage],
    ) -> Result<(), sqlx::Error> {
        let ids: Vec<Uuid> = messages
            .iter()
            .filter(|message| message.attachment.is_some())
            .map(|message| message.id)
            .collect();
        if ids.is_empty() {
            return Ok(());
        }
        let rows: Vec<(Uuid, i32, Option<Vec<u8>>)> = with_pool!(self, |pool| {
            let mut query = QueryBuilder::new(
                "SELECT message_id, duration_ms, thumbnail FROM video_notes WHERE message_id IN (",
            );
            {
                let mut values = query.separated(", ");
                for id in &ids {
                    values.push_bind(*id);
                }
            }
            query.push(")");
            query.build_query_as().fetch_all(pool).await
        })?;
        let mut by_message: HashMap<Uuid, (i32, Option<Vec<u8>>)> = rows
            .into_iter()
            .map(|(id, duration, thumbnail)| (id, (duration, thumbnail)))
            .collect();
        for message in messages.iter_mut() {
            if let Some((duration_ms, thumbnail)) = by_message.remove(&message.id) {
                message.video_note = Some(VideoNote {
                    duration_ms: u32::try_from(duration_ms).unwrap_or(0),
                    thumbnail: encode_thumbnail(thumbnail),
                    listened: false,
                });
            }
        }
        Ok(())
    }

    /// Per-viewer half: `video_note.listened`, set after the cache (like `voice.listened`).
    pub(crate) async fn attach_video_note_listened(
        &self,
        messages: &mut [StoredMessage],
        viewer_id: Option<Uuid>,
    ) -> Result<(), sqlx::Error> {
        let Some(viewer_id) = viewer_id else {
            return Ok(());
        };
        let ids: Vec<Uuid> = messages
            .iter()
            .filter(|message| message.video_note.is_some())
            .map(|message| message.id)
            .collect();
        if ids.is_empty() {
            return Ok(());
        }
        let listened: Vec<Uuid> = with_pool!(self, |pool| {
            let mut query = QueryBuilder::new(
                "SELECT DISTINCT video_note_listens.message_id FROM video_note_listens \
                 JOIN messages ON messages.id = video_note_listens.message_id \
                 WHERE (video_note_listens.user_id = ",
            );
            query.push_bind(viewer_id);
            query.push(" OR messages.sender_id = ");
            query.push_bind(viewer_id);
            query.push(") AND video_note_listens.message_id IN (");
            {
                let mut values = query.separated(", ");
                for id in &ids {
                    values.push_bind(*id);
                }
            }
            query.push(")");
            query.build_query_scalar().fetch_all(pool).await
        })?;
        let listened: HashSet<Uuid> = listened.into_iter().collect();
        for message in messages.iter_mut() {
            if let Some(note) = message.video_note.as_mut() {
                note.listened = listened.contains(&message.id);
            }
        }
        Ok(())
    }
}
