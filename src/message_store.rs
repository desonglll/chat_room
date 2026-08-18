//! Durable message and attachment persistence.

use chrono::{DateTime, Utc};
use sqlx::FromRow;
use uuid::Uuid;

use crate::attachment_storage::StagedUpload;
use crate::models::{Attachment, ReplyPreview, StoredMessage, User};
use crate::state::AppState;

const MESSAGE_SELECT: &str = "SELECT messages.id, messages.room_id, messages.sender_id, \
    messages.sender, COALESCE(sender_user.avatar_emoji, '') AS sender_avatar, messages.content, \
    messages.recalled_at, messages.edited_at, messages.created_at, attachments.id AS attachment_id, \
    attachments.access_key AS attachment_access_key, \
    attachments.file_name AS attachment_file_name, \
    attachments.mime_type AS attachment_mime_type, \
    attachments.size_bytes AS attachment_size_bytes, reply.id AS reply_message_id, \
    reply.sender AS reply_sender, reply.content AS reply_content, \
    reply.recalled_at AS reply_recalled_at, \
    reply_attachment.file_name AS reply_attachment_file_name FROM messages \
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
    pub staged: StagedUpload,
}

pub struct AttachmentMetadata {
    pub file_name: String,
    pub mime_type: String,
    pub size_bytes: i64,
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
    reply_message_id: Option<Uuid>,
    reply_sender: Option<String>,
    reply_content: Option<String>,
    reply_recalled_at: Option<DateTime<Utc>>,
    reply_attachment_file_name: Option<String>,
}

impl MessageRow {
    fn into_message(self) -> StoredMessage {
        let recalled = self.recalled_at.is_some();
        let attachment = (!recalled)
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
        StoredMessage {
            id: self.id,
            room_id: self.room_id,
            sender_id: self.sender_id,
            sender: self.sender,
            sender_avatar: self.sender_avatar,
            content: if recalled {
                String::new()
            } else {
                self.content
            },
            attachment,
            reply_to,
            recalled_at: self.recalled_at,
            edited_at: self.edited_at,
            created_at: self.created_at,
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
        sqlx::query(
            "INSERT INTO messages \
             (id, room_id, sender_id, sender, content, reply_to_id, created_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(id)
        .bind(room_id)
        .bind(sender_id)
        .bind(sender)
        .bind(content)
        .bind(reply_to.as_ref().map(|reply| reply.message_id))
        .bind(created_at)
        .execute(self.pool())
        .await?;

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
        })
    }

    /// Atomically persist an attachment and the chat message that owns it.
    pub async fn store_attachment_message(
        &self,
        room_id: Uuid,
        sender: &User,
        upload: NewAttachment,
        content: &str,
        reply_to: Option<Uuid>,
    ) -> anyhow::Result<StoredMessage> {
        let attachment_id = Uuid::new_v4();
        let access_key = Uuid::new_v4();
        let message_id = Uuid::new_v4();
        let created_at = Utc::now();
        let reply_to = self.reply_preview(room_id, reply_to).await?;
        let NewAttachment {
            file_name,
            mime_type,
            staged,
        } = upload;
        let size_bytes = self
            .attachment_store()
            .commit(staged, attachment_id)
            .await?;
        let mut transaction = self.pool().begin().await?;

        let persisted = async {
            sqlx::query(
                "INSERT INTO attachments \
             (id, access_key, room_id, uploader_id, file_name, mime_type, size_bytes, created_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(attachment_id)
            .bind(access_key)
            .bind(room_id)
            .bind(sender.id)
            .bind(&file_name)
            .bind(&mime_type)
            .bind(size_bytes)
            .bind(created_at)
            .execute(&mut *transaction)
            .await?;

            sqlx::query(
                "INSERT INTO messages \
             (id, room_id, sender_id, sender, content, attachment_id, reply_to_id, created_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(message_id)
            .bind(room_id)
            .bind(sender.id)
            .bind(&sender.username)
            .bind(content)
            .bind(attachment_id)
            .bind(reply_to.as_ref().map(|reply| reply.message_id))
            .bind(created_at)
            .execute(&mut *transaction)
            .await?;
            transaction.commit().await
        }
        .await;
        if let Err(error) = persisted {
            let _ = self.attachment_store().remove(attachment_id).await;
            return Err(error.into());
        }

        Ok(StoredMessage {
            id: message_id,
            room_id,
            sender_id: Some(sender.id),
            sender: sender.username.clone(),
            sender_avatar: sender.avatar_emoji.clone(),
            content: content.to_string(),
            attachment: Some(Attachment {
                id: attachment_id,
                file_name,
                mime_type,
                size_bytes,
                download_url: format!("/api/attachments/{attachment_id}?key={access_key}"),
            }),
            reply_to,
            recalled_at: None,
            edited_at: None,
            created_at,
        })
    }

    pub async fn attachment_metadata(
        &self,
        id: Uuid,
        access_key: Uuid,
    ) -> Result<Option<AttachmentMetadata>, sqlx::Error> {
        let row: Option<(String, String, i64)> = sqlx::query_as(
            "SELECT file_name, mime_type, size_bytes FROM attachments \
             WHERE id = ? AND access_key = ? AND EXISTS (\
               SELECT 1 FROM messages WHERE messages.attachment_id = attachments.id \
               AND messages.recalled_at IS NULL\
             )",
        )
        .bind(id)
        .bind(access_key)
        .fetch_optional(self.pool())
        .await?;
        Ok(
            row.map(|(file_name, mime_type, size_bytes)| AttachmentMetadata {
                file_name,
                mime_type,
                size_bytes,
            }),
        )
    }

    pub(crate) async fn latest_message_cursor(
        &self,
        room_id: Uuid,
    ) -> Result<Option<MessageCursor>, sqlx::Error> {
        let row: Option<(DateTime<Utc>, Uuid)> = sqlx::query_as(
            "SELECT created_at, id FROM messages WHERE room_id = ? \
             ORDER BY created_at DESC, id DESC LIMIT 1",
        )
        .bind(room_id)
        .fetch_optional(self.pool())
        .await?;
        Ok(row.map(|(created_at, id)| MessageCursor { created_at, id }))
    }

    pub(crate) async fn messages_after(
        &self,
        room_id: Uuid,
        cursor: Option<&MessageCursor>,
        limit: i64,
    ) -> Result<Vec<StoredMessage>, sqlx::Error> {
        let limit = limit.clamp(1, 500);
        let query = match cursor {
            Some(_) => format!(
                "{MESSAGE_SELECT} WHERE messages.room_id = ? AND \
                 (messages.created_at > ? OR (messages.created_at = ? AND messages.id > ?)) \
                 ORDER BY messages.created_at ASC, messages.id ASC LIMIT ?"
            ),
            None => format!(
                "{MESSAGE_SELECT} WHERE messages.room_id = ? \
                 ORDER BY messages.created_at ASC, messages.id ASC LIMIT ?"
            ),
        };
        let rows: Vec<MessageRow> = match cursor {
            Some(cursor) => {
                sqlx::query_as(&query)
                    .bind(room_id)
                    .bind(cursor.created_at)
                    .bind(cursor.created_at)
                    .bind(cursor.id)
                    .bind(limit)
                    .fetch_all(self.pool())
                    .await?
            }
            None => {
                sqlx::query_as(&query)
                    .bind(room_id)
                    .bind(limit)
                    .fetch_all(self.pool())
                    .await?
            }
        };
        Ok(rows.into_iter().map(MessageRow::into_message).collect())
    }

    pub(crate) async fn message_history(
        &self,
        room_id: Uuid,
        limit: i64,
        through: Option<&MessageCursor>,
    ) -> Result<Vec<StoredMessage>, sqlx::Error> {
        let limit = limit.clamp(1, 500);
        let query = match through {
            Some(_) => format!(
                "{MESSAGE_SELECT} WHERE messages.room_id = ? AND \
                 (messages.created_at < ? OR (messages.created_at = ? AND messages.id <= ?)) \
                 ORDER BY messages.created_at DESC, messages.id DESC LIMIT ?"
            ),
            None => format!(
                "{MESSAGE_SELECT} WHERE messages.room_id = ? \
                 ORDER BY messages.created_at DESC, messages.id DESC LIMIT ?"
            ),
        };
        let mut rows: Vec<MessageRow> = match through {
            Some(cursor) => {
                sqlx::query_as(&query)
                    .bind(room_id)
                    .bind(cursor.created_at)
                    .bind(cursor.created_at)
                    .bind(cursor.id)
                    .bind(limit)
                    .fetch_all(self.pool())
                    .await?
            }
            None => {
                sqlx::query_as(&query)
                    .bind(room_id)
                    .bind(limit)
                    .fetch_all(self.pool())
                    .await?
            }
        };
        rows.reverse();
        Ok(rows.into_iter().map(MessageRow::into_message).collect())
    }

    async fn reply_preview(
        &self,
        room_id: Uuid,
        message_id: Option<Uuid>,
    ) -> Result<Option<ReplyPreview>, sqlx::Error> {
        let Some(message_id) = message_id else {
            return Ok(None);
        };
        let row: Option<ReplySourceRow> = sqlx::query_as(
            "SELECT messages.id, messages.sender, messages.content, attachments.file_name, \
                    messages.recalled_at \
             FROM messages LEFT JOIN attachments ON attachments.id = messages.attachment_id \
             WHERE messages.id = ? AND messages.room_id = ?",
        )
        .bind(message_id)
        .bind(room_id)
        .fetch_optional(self.pool())
        .await?;
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
