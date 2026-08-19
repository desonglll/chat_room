//! Durable message and attachment persistence.

use chrono::{DateTime, Utc};
use sqlx::FromRow;
use uuid::Uuid;

use crate::attachment_storage::StagedUpload;
use crate::models::{Attachment, ForwardedFrom, ReplyPreview, StoredMessage, User};
use crate::state::{with_pool, AppState};

const MESSAGE_SELECT: &str = "SELECT messages.id, messages.room_id, messages.sender_id, \
    messages.sender, COALESCE(sender_user.avatar_emoji, '') AS sender_avatar, messages.content, \
    messages.recalled_at, messages.edited_at, messages.created_at, attachments.id AS attachment_id, \
    attachments.access_key AS attachment_access_key, \
    attachments.file_name AS attachment_file_name, \
    attachments.mime_type AS attachment_mime_type, \
    attachments.size_bytes AS attachment_size_bytes, \
    attachments.is_sensitive AS attachment_is_sensitive, reply.id AS reply_message_id, \
    reply.sender AS reply_sender, reply.content AS reply_content, \
    reply.recalled_at AS reply_recalled_at, \
    reply_attachment.file_name AS reply_attachment_file_name, \
    messages.forwarded_from_sender, messages.forwarded_from_room_name FROM messages \
    LEFT JOIN attachments ON attachments.id = messages.attachment_id \
    LEFT JOIN users AS sender_user ON sender_user.id = messages.sender_id \
    LEFT JOIN messages AS reply ON reply.id = messages.reply_to_id \
    LEFT JOIN attachments AS reply_attachment ON reply_attachment.id = reply.attachment_id";

type ReplySourceRow = (Uuid, String, String, Option<String>, Option<DateTime<Utc>>);

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MessageCursor {
    pub created_at: DateTime<Utc>,
    pub id: Uuid,
}

pub struct NewAttachment {
    pub file_name: String,
    pub mime_type: String,
    pub is_sensitive: bool,
    pub staged: StagedUpload,
}

pub struct AttachmentMetadata {
    pub file_name: String,
    pub mime_type: String,
    pub size_bytes: i64,
    pub storage_key: Option<String>,
}

#[derive(FromRow)]
struct MessageRow {
    id: Uuid,
    room_id: Uuid,
    sender_id: Option<Uuid>,
    sender: String,
    sender_avatar: String,
    content: String,
    recalled_at: Option<DateTime<Utc>>,
    edited_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    attachment_id: Option<Uuid>,
    attachment_access_key: Option<Uuid>,
    attachment_file_name: Option<String>,
    attachment_mime_type: Option<String>,
    attachment_size_bytes: Option<i64>,
    attachment_is_sensitive: Option<bool>,
    reply_message_id: Option<Uuid>,
    reply_sender: Option<String>,
    reply_content: Option<String>,
    reply_recalled_at: Option<DateTime<Utc>>,
    reply_attachment_file_name: Option<String>,
    forwarded_from_sender: Option<String>,
    forwarded_from_room_name: Option<String>,
}

impl MessageRow {
    /// `viewer_id` decides recall redaction: the sender of a recalled message keeps
    /// seeing their own draft (so they can re-edit it); every other viewer sees it blanked.
    fn into_message(self, viewer_id: Option<Uuid>) -> StoredMessage {
        let recalled = self.recalled_at.is_some();
        let redact = recalled && viewer_id != self.sender_id;
        let attachment = (!redact)
            .then_some(self.attachment_id)
            .flatten()
            .and_then(|id| {
                Some(Attachment {
                    id,
                    file_name: self.attachment_file_name?,
                    mime_type: self.attachment_mime_type?,
                    size_bytes: self.attachment_size_bytes?,
                    download_url: format!(
                        "/api/attachments/{id}?key={}",
                        self.attachment_access_key?
                    ),
                    is_sensitive: self.attachment_is_sensitive.unwrap_or(false),
                })
            });
        let reply_to = self.reply_message_id.and_then(|message_id| {
            let recalled = self.reply_recalled_at.is_some();
            Some(ReplyPreview {
                message_id,
                sender: self.reply_sender?,
                content: if recalled {
                    String::new()
                } else {
                    self.reply_content?
                },
                attachment_file_name: if recalled {
                    None
                } else {
                    self.reply_attachment_file_name
                },
                recalled,
            })
        });
        let forwarded_from = self.forwarded_from_sender.and_then(|sender| {
            Some(ForwardedFrom {
                sender,
                room_name: self.forwarded_from_room_name?,
            })
        });
        StoredMessage {
            id: self.id,
            room_id: self.room_id,
            sender_id: self.sender_id,
            sender: self.sender,
            sender_avatar: self.sender_avatar,
            content: if redact { String::new() } else { self.content },
            attachment,
            reply_to,
            recalled_at: self.recalled_at,
            edited_at: self.edited_at,
            created_at: self.created_at,
            forwarded_from,
        }
    }
}

impl AppState {
    /// Store a text message before it is forwarded to room participants.
    pub async fn store_message(
        &self,
        room_id: Uuid,
        sender_id: Uuid,
        sender: &str,
        sender_avatar: &str,
        content: &str,
        reply_to: Option<Uuid>,
    ) -> Result<StoredMessage, sqlx::Error> {
        let id = Uuid::new_v4();
        let created_at = Utc::now();
        let reply_to = self.reply_preview(room_id, reply_to).await?;
        with_pool!(self, |pool| { sqlx::query(
            "INSERT INTO messages \
             (id, room_id, sender_id, sender, content, reply_to_id, created_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(id)
        .bind(room_id)
        .bind(sender_id)
        .bind(sender)
        .bind(content)
        .bind(reply_to.as_ref().map(|reply| reply.message_id))
        .bind(created_at)
        .execute(pool)
        .await
        .map(|_| ()) })?;

        Ok(StoredMessage {
            id,
            room_id,
            sender_id: Some(sender_id),
            sender: sender.to_string(),
            sender_avatar: sender_avatar.to_string(),
            content: content.to_string(),
            attachment: None,
            reply_to,
            recalled_at: None,
            edited_at: None,
            created_at,
            forwarded_from: None,
        })
    }

    /// Copy a still-visible message (and its attachment, if any) into another room as
    /// a new message sent by `forwarder`. Returns `None` if the source message doesn't
    /// exist, isn't in `source_room_id`, or has been recalled.
    pub async fn forward_message(
        &self,
        source_message_id: Uuid,
        source_room_id: Uuid,
        target_room_id: Uuid,
        forwarder: &User,
    ) -> Result<Option<StoredMessage>, sqlx::Error> {
        let source: Option<(String, String, Option<Uuid>, String)> = with_pool!(self, |pool| {
            sqlx::query_as(
                "SELECT messages.sender, messages.content, messages.attachment_id, rooms.name \
                 FROM messages JOIN rooms ON rooms.id = messages.room_id \
                 WHERE messages.id = $1 AND messages.room_id = $2 AND messages.recalled_at IS NULL",
            )
            .bind(source_message_id)
            .bind(source_room_id)
            .fetch_optional(pool)
            .await
        })?;
        let Some((source_sender, source_content, attachment_id, source_room_name)) = source
        else {
            return Ok(None);
        };
        let id = Uuid::new_v4();
        let created_at = Utc::now();
        let forwarder_display_name = self.resolve_display_name(target_room_id, forwarder).await;
        with_pool!(self, |pool| {
            sqlx::query(
                "INSERT INTO messages \
                 (id, room_id, sender_id, sender, content, attachment_id, \
                  forwarded_from_sender, forwarded_from_room_name, created_at) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
            )
            .bind(id)
            .bind(target_room_id)
            .bind(forwarder.id)
            .bind(&forwarder_display_name)
            .bind(&source_content)
            .bind(attachment_id)
            .bind(&source_sender)
            .bind(&source_room_name)
            .bind(created_at)
            .execute(pool)
            .await
            .map(|_| ())
        })?;
        self.message_by_id(id, Some(forwarder.id)).await
    }

    pub async fn record_message_mentions(
        &self,
        message_id: Uuid,
        mentioned_user_ids: &[Uuid],
    ) -> Result<(), sqlx::Error> {
        let created_at = Utc::now();
        with_pool!(self, |pool| {
            for user_id in mentioned_user_ids {
                sqlx::query(
                    "INSERT INTO message_mentions (message_id, mentioned_user_id, created_at) \
                     VALUES ($1, $2, $3)",
                )
                .bind(message_id)
                .bind(user_id)
                .bind(created_at)
                .execute(pool)
                .await?;
            }
            Ok::<_, sqlx::Error>(())
        })
    }

    pub async fn message_room_id(&self, id: Uuid) -> Result<Option<Uuid>, sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query_scalar("SELECT room_id FROM messages WHERE id = $1")
                .bind(id)
                .fetch_optional(pool)
                .await
        })
    }

    pub(crate) async fn message_by_id(
        &self,
        id: Uuid,
        viewer_id: Option<Uuid>,
    ) -> Result<Option<StoredMessage>, sqlx::Error> {
        let query = format!("{MESSAGE_SELECT} WHERE messages.id = $1");
        let row: Option<MessageRow> = with_pool!(self, |pool| {
            sqlx::query_as(&query).bind(id).fetch_optional(pool).await
        })?;
        Ok(row.map(|row| row.into_message(viewer_id)))
    }

    pub async fn attachment_metadata(
        &self,
        id: Uuid,
        access_key: Uuid,
    ) -> Result<Option<AttachmentMetadata>, sqlx::Error> {
        let row: Option<(String, String, i64, Option<String>)> = with_pool!(self, |pool| { sqlx::query_as(
            "SELECT file_name, mime_type, size_bytes, storage_key FROM attachments \
             WHERE id = $1 AND access_key = $2 AND EXISTS (\
               SELECT 1 FROM messages WHERE messages.attachment_id = attachments.id \
               AND messages.recalled_at IS NULL\
             )",
        )
        .bind(id)
        .bind(access_key)
        .fetch_optional(pool)
        .await })?;
        Ok(row.map(
            |(file_name, mime_type, size_bytes, storage_key)| AttachmentMetadata {
                file_name,
                mime_type,
                size_bytes,
                storage_key,
            },
        ))
    }

    pub(crate) async fn latest_message_cursor(
        &self,
        room_id: Uuid,
    ) -> Result<Option<MessageCursor>, sqlx::Error> {
        let row: Option<(DateTime<Utc>, Uuid)> = with_pool!(self, |pool| { sqlx::query_as(
            "SELECT created_at, id FROM messages WHERE room_id = $1 \
             ORDER BY created_at DESC, id DESC LIMIT 1",
        )
        .bind(room_id)
        .fetch_optional(pool)
        .await })?;
        Ok(row.map(|(created_at, id)| MessageCursor { created_at, id }))
    }

    pub(crate) async fn messages_after(
        &self,
        room_id: Uuid,
        cursor: Option<&MessageCursor>,
        limit: i64,
        viewer_id: Option<Uuid>,
    ) -> Result<Vec<StoredMessage>, sqlx::Error> {
        let limit = limit.clamp(1, 500);
        let query = match cursor {
            Some(_) => format!(
                "{MESSAGE_SELECT} WHERE messages.room_id = $1 AND \
                 (messages.created_at > $2 OR (messages.created_at = $3 AND messages.id > $4)) \
                 ORDER BY messages.created_at ASC, messages.id ASC LIMIT $5"
            ),
            None => format!(
                "{MESSAGE_SELECT} WHERE messages.room_id = $1 \
                 ORDER BY messages.created_at ASC, messages.id ASC LIMIT $2"
            ),
        };
        let rows: Vec<MessageRow> = with_pool!(self, |pool| { match cursor {
            Some(cursor) => {
                sqlx::query_as(&query)
                    .bind(room_id)
                    .bind(cursor.created_at)
                    .bind(cursor.created_at)
                    .bind(cursor.id)
                    .bind(limit)
                    .fetch_all(pool)
                    .await
            }
            None => {
                sqlx::query_as(&query)
                    .bind(room_id)
                    .bind(limit)
                    .fetch_all(pool)
                    .await
            }
        } })?;
        Ok(rows
            .into_iter()
            .map(|row| row.into_message(viewer_id))
            .collect())
    }

    pub(crate) async fn message_history(
        &self,
        room_id: Uuid,
        limit: i64,
        through: Option<&MessageCursor>,
        viewer_id: Option<Uuid>,
    ) -> Result<Vec<StoredMessage>, sqlx::Error> {
        let limit = limit.clamp(1, 500);
        let query = match through {
            Some(_) => format!(
                "{MESSAGE_SELECT} WHERE messages.room_id = $1 AND \
                 (messages.created_at < $2 OR (messages.created_at = $3 AND messages.id <= $4)) \
                 ORDER BY messages.created_at DESC, messages.id DESC LIMIT $5"
            ),
            None => format!(
                "{MESSAGE_SELECT} WHERE messages.room_id = $1 \
                 ORDER BY messages.created_at DESC, messages.id DESC LIMIT $2"
            ),
        };
        let mut rows: Vec<MessageRow> = with_pool!(self, |pool| { match through {
            Some(cursor) => {
                sqlx::query_as(&query)
                    .bind(room_id)
                    .bind(cursor.created_at)
                    .bind(cursor.created_at)
                    .bind(cursor.id)
                    .bind(limit)
                    .fetch_all(pool)
                    .await
            }
            None => {
                sqlx::query_as(&query)
                    .bind(room_id)
                    .bind(limit)
                    .fetch_all(pool)
                    .await
            }
        } })?;
        rows.reverse();
        Ok(rows
            .into_iter()
            .map(|row| row.into_message(viewer_id))
            .collect())
    }

    pub(crate) async fn reply_preview(
        &self,
        room_id: Uuid,
        message_id: Option<Uuid>,
    ) -> Result<Option<ReplyPreview>, sqlx::Error> {
        let Some(message_id) = message_id else {
            return Ok(None);
        };
        let row: Option<ReplySourceRow> = with_pool!(self, |pool| { sqlx::query_as(
            "SELECT messages.id, messages.sender, messages.content, attachments.file_name, \
                    messages.recalled_at \
             FROM messages LEFT JOIN attachments ON attachments.id = messages.attachment_id \
             WHERE messages.id = $1 AND messages.room_id = $2",
        )
        .bind(message_id)
        .bind(room_id)
        .fetch_optional(pool)
        .await })?;
        Ok(row.map(
            |(message_id, sender, content, attachment_file_name, recalled_at)| ReplyPreview {
                message_id,
                sender,
                content: if recalled_at.is_some() {
                    String::new()
                } else {
                    content
                },
                attachment_file_name: if recalled_at.is_some() {
                    None
                } else {
                    attachment_file_name
                },
                recalled: recalled_at.is_some(),
            },
        ))
    }
}
