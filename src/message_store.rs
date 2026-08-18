//! Durable message and attachment persistence.

use chrono::{DateTime, Utc};
use sqlx::FromRow;
use uuid::Uuid;

use crate::models::{Attachment, StoredMessage};
use crate::state::AppState;

const MESSAGE_COLUMNS: &str = "messages.id, messages.room_id, messages.sender_id, \
    messages.sender, messages.content, messages.created_at, attachments.id AS attachment_id, \
    attachments.access_key AS attachment_access_key, \
    attachments.file_name AS attachment_file_name, \
    attachments.mime_type AS attachment_mime_type, \
    attachments.size_bytes AS attachment_size_bytes";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MessageCursor {
    pub created_at: DateTime<Utc>,
    pub id: Uuid,
}

pub struct NewAttachment {
    pub file_name: String,
    pub mime_type: String,
    pub data: Vec<u8>,
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
    content: String,
    created_at: DateTime<Utc>,
    attachment_id: Option<Uuid>,
    attachment_access_key: Option<Uuid>,
    attachment_file_name: Option<String>,
    attachment_mime_type: Option<String>,
    attachment_size_bytes: Option<i64>,
}

impl MessageRow {
    fn into_message(self) -> StoredMessage {
        let attachment = self.attachment_id.and_then(|id| {
            Some(Attachment {
                id,
                file_name: self.attachment_file_name?,
                mime_type: self.attachment_mime_type?,
                size_bytes: self.attachment_size_bytes?,
                download_url: format!("/api/attachments/{id}?key={}", self.attachment_access_key?),
            })
        });
        StoredMessage {
            id: self.id,
            room_id: self.room_id,
            sender_id: self.sender_id,
            sender: self.sender,
            content: self.content,
            attachment,
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
        content: &str,
    ) -> Result<StoredMessage, sqlx::Error> {
        let id = Uuid::new_v4();
        let created_at = Utc::now();
        sqlx::query(
            "INSERT INTO messages (id, room_id, sender_id, sender, content, created_at) \
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(id)
        .bind(room_id)
        .bind(sender_id)
        .bind(sender)
        .bind(content)
        .bind(created_at)
        .execute(self.pool())
        .await?;

        Ok(StoredMessage {
            id,
            room_id,
            sender_id: Some(sender_id),
            sender: sender.to_string(),
            content: content.to_string(),
            attachment: None,
            created_at,
        })
    }

    /// Atomically persist an attachment and the chat message that owns it.
    pub async fn store_attachment_message(
        &self,
        room_id: Uuid,
        sender_id: Uuid,
        sender: &str,
        upload: NewAttachment,
    ) -> Result<StoredMessage, sqlx::Error> {
        let attachment_id = Uuid::new_v4();
        let access_key = Uuid::new_v4();
        let message_id = Uuid::new_v4();
        let created_at = Utc::now();
        let size_bytes = upload.data.len() as i64;
        let mut transaction = self.pool().begin().await?;

        sqlx::query(
            "INSERT INTO attachments \
             (id, access_key, room_id, uploader_id, file_name, mime_type, size_bytes, data, created_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(attachment_id)
        .bind(access_key)
        .bind(room_id)
        .bind(sender_id)
        .bind(&upload.file_name)
        .bind(&upload.mime_type)
        .bind(size_bytes)
        .bind(&upload.data)
        .bind(created_at)
        .execute(&mut *transaction)
        .await?;

        sqlx::query(
            "INSERT INTO messages \
             (id, room_id, sender_id, sender, content, attachment_id, created_at) \
             VALUES (?, ?, ?, ?, '', ?, ?)",
        )
        .bind(message_id)
        .bind(room_id)
        .bind(sender_id)
        .bind(sender)
        .bind(attachment_id)
        .bind(created_at)
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;

        Ok(StoredMessage {
            id: message_id,
            room_id,
            sender_id: Some(sender_id),
            sender: sender.to_string(),
            content: String::new(),
            attachment: Some(Attachment {
                id: attachment_id,
                file_name: upload.file_name,
                mime_type: upload.mime_type,
                size_bytes,
                download_url: format!("/api/attachments/{attachment_id}?key={access_key}"),
            }),
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
             WHERE id = ? AND access_key = ?",
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

    pub async fn attachment_chunk(
        &self,
        id: Uuid,
        access_key: Uuid,
        start: i64,
        length: i64,
    ) -> Result<Option<Vec<u8>>, sqlx::Error> {
        sqlx::query_scalar(
            "SELECT substr(data, ? + 1, ?) FROM attachments WHERE id = ? AND access_key = ?",
        )
        .bind(start)
        .bind(length)
        .bind(id)
        .bind(access_key)
        .fetch_optional(self.pool())
        .await
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
                "SELECT {MESSAGE_COLUMNS} FROM messages LEFT JOIN attachments \
                 ON attachments.id = messages.attachment_id WHERE messages.room_id = ? AND \
                 (messages.created_at > ? OR (messages.created_at = ? AND messages.id > ?)) \
                 ORDER BY messages.created_at ASC, messages.id ASC LIMIT ?"
            ),
            None => format!(
                "SELECT {MESSAGE_COLUMNS} FROM messages LEFT JOIN attachments \
                 ON attachments.id = messages.attachment_id WHERE messages.room_id = ? \
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
                "SELECT {MESSAGE_COLUMNS} FROM messages LEFT JOIN attachments \
                 ON attachments.id = messages.attachment_id WHERE messages.room_id = ? AND \
                 (messages.created_at < ? OR (messages.created_at = ? AND messages.id <= ?)) \
                 ORDER BY messages.created_at DESC, messages.id DESC LIMIT ?"
            ),
            None => format!(
                "SELECT {MESSAGE_COLUMNS} FROM messages LEFT JOIN attachments \
                 ON attachments.id = messages.attachment_id WHERE messages.room_id = ? \
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
}
