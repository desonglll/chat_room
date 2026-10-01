//! Durable account-level message cursor used by cross-chat notifications.

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::FromRow;
use uuid::Uuid;

use crate::conversations::preview_media::{
    preview_media_columns, preview_media_kind, PreviewMediaColumns,
};
use crate::state::{with_pool, AppState};

#[derive(Clone)]
pub(crate) struct AccountMessageCursor {
    pub created_at: DateTime<Utc>,
    pub id: Uuid,
}

#[derive(FromRow)]
struct AccountEventRow {
    message_id: Uuid,
    room_id: Uuid,
    room_name: String,
    conversation_kind: String,
    conversation_title: String,
    sender_id: Option<Uuid>,
    sender: String,
    content: String,
    attachment_file_name: Option<String>,
    created_at: DateTime<Utc>,
    is_mention: bool,
    #[sqlx(flatten)]
    media: PreviewMediaColumns,
}

#[derive(Serialize)]
pub(crate) struct AccountMessageEvent {
    #[serde(rename = "type")]
    kind: &'static str,
    pub message_id: Uuid,
    pub room_id: Uuid,
    pub room_name: String,
    pub conversation_kind: String,
    pub conversation_title: String,
    pub sender_id: Option<Uuid>,
    pub sender: String,
    pub content: String,
    pub attachment_file_name: Option<String>,
    /// TG-802: same vocabulary as `MessagePreview::media_kind`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub media_kind: Option<&'static str>,
    pub timestamp: DateTime<Utc>,
    pub is_mention: bool,
}

impl AccountEventRow {
    fn cursor(&self) -> AccountMessageCursor {
        AccountMessageCursor {
            created_at: self.created_at,
            id: self.message_id,
        }
    }

    fn into_event(self) -> AccountMessageEvent {
        AccountMessageEvent {
            kind: "new_message",
            message_id: self.message_id,
            room_id: self.room_id,
            room_name: self.room_name,
            conversation_kind: self.conversation_kind,
            conversation_title: self.conversation_title,
            sender_id: self.sender_id,
            sender: self.sender,
            content: self.content,
            attachment_file_name: self.attachment_file_name,
            media_kind: preview_media_kind(&self.media),
            timestamp: self.created_at,
            is_mention: self.is_mention,
        }
    }
}

impl AppState {
    pub(crate) async fn latest_account_message_cursor(
        &self,
        user_id: Uuid,
    ) -> Result<Option<AccountMessageCursor>, sqlx::Error> {
        let row: Option<(DateTime<Utc>, Uuid)> = with_pool!(self, |pool| {
            sqlx::query_as(
            "SELECT messages.created_at, messages.id FROM messages \
             JOIN chat_members ON chat_members.room_id = messages.room_id \
             WHERE chat_members.user_id = $1 AND chat_members.status = 'active' \
             AND messages.created_at >= COALESCE(chat_members.joined_at, chat_members.requested_at) \
             ORDER BY messages.created_at DESC, messages.id DESC LIMIT 1",
        )
        .bind(user_id)
        .fetch_optional(pool)
        .await
        })?;
        Ok(row.map(|(created_at, id)| AccountMessageCursor { created_at, id }))
    }

    pub(crate) async fn account_messages_after(
        &self,
        user_id: Uuid,
        cursor: Option<&AccountMessageCursor>,
    ) -> Result<Vec<(AccountMessageCursor, AccountMessageEvent)>, sqlx::Error> {
        let cursor_clause = if cursor.is_some() {
            "AND (messages.created_at > $2 OR (messages.created_at = $3 AND messages.id > $4))"
        } else {
            ""
        };
        let media_columns = preview_media_columns!("messages", "attachments");
        let sql = format!(
            "SELECT messages.id AS message_id, messages.room_id, chats.title AS room_name, \
             CASE WHEN direct.room_id IS NULL THEN 'group' ELSE 'direct' END \
               AS conversation_kind, \
             CASE WHEN direct.room_id IS NULL THEN chats.title \
               ELSE COALESCE(NULLIF(peer.display_name, ''), peer.username) END \
               AS conversation_title, \
             messages.sender_id, messages.sender, messages.content, \
             attachments.file_name AS attachment_file_name, messages.created_at, \
             (mention.message_id IS NOT NULL) AS is_mention, {media_columns} \
             FROM messages JOIN chats ON chats.id = messages.room_id \
             JOIN chat_members ON chat_members.room_id = messages.room_id \
             LEFT JOIN direct_conversations AS direct ON direct.room_id = chats.id \
             LEFT JOIN users AS peer ON peer.id = CASE \
               WHEN direct.user_low_id = $1 THEN direct.user_high_id \
               WHEN direct.user_high_id = $1 THEN direct.user_low_id ELSE NULL END \
             LEFT JOIN attachments ON attachments.id = messages.attachment_id \
             LEFT JOIN message_mentions AS mention ON mention.message_id = messages.id \
               AND mention.mentioned_user_id = $1 \
             WHERE chat_members.user_id = $1 AND chat_members.status = 'active' \
             AND messages.created_at >= COALESCE(chat_members.joined_at, chat_members.requested_at) \
             AND messages.recalled_at IS NULL {cursor_clause} \
             ORDER BY messages.created_at ASC, messages.id ASC LIMIT 200"
        );
        let rows = with_pool!(self, |pool| {
            let query = sqlx::query_as::<_, AccountEventRow>(&sql).bind(user_id);
            match cursor {
                Some(cursor) => {
                    query
                        .bind(cursor.created_at)
                        .bind(cursor.created_at)
                        .bind(cursor.id)
                        .fetch_all(pool)
                        .await
                }
                None => query.fetch_all(pool).await,
            }
        })?;
        Ok(rows
            .into_iter()
            .map(|row| (row.cursor(), row.into_event()))
            .collect())
    }
}
