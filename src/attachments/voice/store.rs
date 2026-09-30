//! Voice persistence: the send transaction, the listened marks, and the two post-load steps
//! that add `StoredMessage::voice` (static, cacheable) and its per-viewer `listened` flag.

use std::collections::{HashMap, HashSet};

use chrono::Utc;
use sqlx::QueryBuilder;
use uuid::Uuid;

use super::model::{
    pack_waveform, unpack_waveform, DurationSource, Listened, VoiceError, VoiceNote,
    MEDIA_KIND_VOICE,
};
use crate::models::{Attachment, StoredMessage, User};
use crate::state::{with_pool, AppState};

/// A validated voice message, ready to be written.
pub(crate) struct NewVoice {
    pub file_name: String,
    pub mime_type: String,
    pub size_bytes: i64,
    pub content_hash: String,
    pub storage_key: String,
    pub duration_ms: u32,
    pub duration_source: DurationSource,
    pub waveform: Vec<u8>,
    pub reply_to: Option<Uuid>,
}

impl AppState {
    /// Write the attachment, the `media_kind = 'voice'` message and its `voice_notes` row in
    /// one transaction (the attachment finaliser plus one insert), so no reader ever sees a
    /// voice message without its waveform. The send permission is re-checked inside it.
    pub(crate) async fn insert_voice_message(
        &self,
        room_id: Uuid,
        sender: &User,
        sender_display_name: &str,
        voice: NewVoice,
    ) -> Result<StoredMessage, VoiceError> {
        let attachment_id = Uuid::new_v4();
        let access_key = Uuid::new_v4();
        let message_id = Uuid::new_v4();
        let created_at = Utc::now();
        let reply_to = self.reply_preview(room_id, voice.reply_to).await?;
        let packed = pack_waveform(&voice.waveform);
        let duration_ms = i32::try_from(voice.duration_ms)
            .map_err(|_| VoiceError::Invalid("invalid_duration"))?;
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
                .bind(&voice.file_name)
                .bind(&voice.mime_type)
                .bind(voice.size_bytes)
                .bind(false)
                .bind(created_at)
                .bind(&voice.content_hash)
                .bind(&voice.storage_key)
                .execute(&mut *tx)
                .await?;
                sqlx::query(
                    "INSERT INTO messages (id, room_id, sender_id, sender, content, \
                     attachment_id, reply_to_id, media_kind, created_at) \
                     VALUES ($1, $2, $3, $4, '', $5, $6, $7, $8)",
                )
                .bind(message_id)
                .bind(room_id)
                .bind(sender.id)
                .bind(sender_display_name)
                .bind(attachment_id)
                .bind(reply_to.as_ref().map(|reply| reply.message_id))
                .bind(MEDIA_KIND_VOICE)
                .bind(created_at)
                .execute(&mut *tx)
                .await?;
                sqlx::query(
                    "INSERT INTO voice_notes \
                     (message_id, duration_ms, waveform, duration_source, created_at) \
                     VALUES ($1, $2, $3, $4, $5)",
                )
                .bind(message_id)
                .bind(duration_ms)
                .bind(&packed)
                .bind(voice.duration_source.as_str())
                .bind(created_at)
                .execute(&mut *tx)
                .await?;
                sqlx::query(
                    "UPDATE attachments SET orphaned_at = NULL \
                     WHERE storage_key = $1 AND orphaned_at IS NOT NULL",
                )
                .bind(&voice.storage_key)
                .execute(&mut *tx)
                .await?;
                tx.commit().await
            }
            .await
        });
        match persisted {
            Ok(()) => {}
            Err(sqlx::Error::RowNotFound) => return Err(VoiceError::Forbidden),
            Err(error) => {
                tracing::warn!(
                    "voice message transaction failed; retained content object {}: {error}",
                    voice.storage_key
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
                file_name: voice.file_name,
                mime_type: voice.mime_type,
                size_bytes: voice.size_bytes,
                download_url: format!("/api/attachments/{attachment_id}?key={access_key}"),
                is_sensitive: false,
            }),
            reply_to,
            created_at,
            media_kind: Some(MEDIA_KIND_VOICE.to_string()),
            voice: Some(VoiceNote {
                duration_ms: voice.duration_ms,
                waveform: voice.waveform,
                listened: false,
            }),
            ..Default::default()
        })
    }

    /// The facts a listened mark needs, or `NotFound` when the message is not a live voice
    /// message in a live chat where `user_id` is an active member — one answer for all, so
    /// a voice message's existence never leaks across the chat boundary.
    pub(crate) async fn voice_listen_target(
        &self,
        message_id: Uuid,
        user_id: Uuid,
    ) -> Result<(Uuid, Option<Uuid>), VoiceError> {
        let row: Option<(Uuid, Option<Uuid>)> = with_pool!(self, |pool| {
            sqlx::query_as(
                "SELECT messages.room_id, messages.sender_id FROM voice_notes \
                 JOIN messages ON messages.id = voice_notes.message_id \
                   AND messages.recalled_at IS NULL \
                 JOIN chats ON chats.id = messages.room_id AND chats.deleted_at IS NULL \
                 WHERE voice_notes.message_id = $1 AND EXISTS (SELECT 1 FROM chat_members \
                   WHERE chat_members.room_id = messages.room_id \
                     AND chat_members.user_id = $2 AND chat_members.status = 'active')",
            )
            .bind(message_id)
            .bind(user_id)
            .fetch_optional(pool)
            .await
        })?;
        row.ok_or(VoiceError::NotFound)
    }

    /// Record that `user_id` played the message. `Some` only the first time, so a replay
    /// never re-broadcasts.
    pub(crate) async fn record_voice_listen(
        &self,
        message_id: Uuid,
        room_id: Uuid,
        sender_id: Option<Uuid>,
        user_id: Uuid,
    ) -> Result<Option<Listened>, VoiceError> {
        let inserted = with_pool!(self, |pool| {
            sqlx::query(
                "INSERT INTO voice_listens (message_id, user_id, listened_at) VALUES ($1, $2, $3) \
                 ON CONFLICT (message_id, user_id) DO NOTHING",
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

    /// Static half: `voice` (duration, waveform) for every voice message whose attachment
    /// this viewer can see. Viewer-independent, so it runs before the Redis history cache.
    pub(crate) async fn attach_message_voice(
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
        let rows: Vec<(Uuid, i32, Vec<u8>)> = with_pool!(self, |pool| {
            let mut query = QueryBuilder::new(
                "SELECT message_id, duration_ms, waveform FROM voice_notes WHERE message_id IN (",
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
        let mut by_message: HashMap<Uuid, (i32, Vec<u8>)> = rows
            .into_iter()
            .map(|(id, duration, waveform)| (id, (duration, waveform)))
            .collect();
        for message in messages.iter_mut() {
            if let Some((duration_ms, packed)) = by_message.remove(&message.id) {
                message.voice = Some(VoiceNote {
                    duration_ms: u32::try_from(duration_ms).unwrap_or(0),
                    waveform: unpack_waveform(&packed),
                    listened: false,
                });
            }
        }
        Ok(())
    }

    /// Per-viewer half: `voice.listened`, set after the cache (like polls' `chosen`). A row
    /// by the viewer means the viewer played it; any row on the viewer's own message means
    /// someone else did (a sender never has a row for their own message).
    pub(crate) async fn attach_voice_listened(
        &self,
        messages: &mut [StoredMessage],
        viewer_id: Option<Uuid>,
    ) -> Result<(), sqlx::Error> {
        let Some(viewer_id) = viewer_id else {
            return Ok(());
        };
        let ids: Vec<Uuid> = messages
            .iter()
            .filter(|message| message.voice.is_some())
            .map(|message| message.id)
            .collect();
        if ids.is_empty() {
            return Ok(());
        }
        let listened: Vec<Uuid> = with_pool!(self, |pool| {
            let mut query = QueryBuilder::new(
                "SELECT DISTINCT voice_listens.message_id FROM voice_listens \
                 JOIN messages ON messages.id = voice_listens.message_id WHERE (voice_listens.user_id = ",
            );
            query.push_bind(viewer_id);
            query.push(" OR messages.sender_id = ");
            query.push_bind(viewer_id);
            query.push(") AND voice_listens.message_id IN (");
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
            if let Some(voice) = message.voice.as_mut() {
                voice.listened = listened.contains(&message.id);
            }
        }
        Ok(())
    }

    /// The other participant of a private chat, for the TG-505 voice privacy rule.
    pub(crate) async fn private_chat_peer_id(
        &self,
        room_id: Uuid,
        viewer_id: Uuid,
    ) -> Result<Option<Uuid>, sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query_scalar(
                "SELECT CASE WHEN user_low_id = $2 THEN user_high_id ELSE user_low_id END \
                 FROM direct_conversations WHERE room_id = $1",
            )
            .bind(room_id)
            .bind(viewer_id)
            .fetch_optional(pool)
            .await
        })
    }
}
