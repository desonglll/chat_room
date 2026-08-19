//! Durable account-level message cursor used by cross-room notifications.

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::FromRow;
use uuid::Uuid;

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
    sender_id: Option<Uuid>,
    sender: String,
    content: String,
    attachment_file_name: Option<String>,
    created_at: DateTime<Utc>,
    is_mention: bool,
}

#[derive(Serialize)]
pub(crate) struct AccountMessageEvent {
    #[serde(rename = "type")]
    kind: &'static str,
    pub message_id: Uuid,
    pub room_id: Uuid,
    pub room_name: String,
    pub sender_id: Option<Uuid>,
    pub sender: String,
    pub content: String,
    pub attachment_file_name: Option<String>,
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
            sender_id: self.sender_id,
            sender: self.sender,
            content: self.content,
            attachment_file_name: self.attachment_file_name,
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
             JOIN room_memberships ON room_memberships.room_id = messages.room_id \
             WHERE room_memberships.user_id = $1 AND room_memberships.status = 'active' \
             AND messages.created_at >= COALESCE(room_memberships.joined_at, room_memberships.requested_at) \
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
        let sql = format!(
            "SELECT messages.id AS message_id, messages.room_id, rooms.name AS room_name, \
             messages.sender_id, messages.sender, messages.content, \
             attachments.file_name AS attachment_file_name, messages.created_at, \
             (mention.message_id IS NOT NULL) AS is_mention \
             FROM messages JOIN rooms ON rooms.id = messages.room_id \
             JOIN room_memberships ON room_memberships.room_id = messages.room_id \
             LEFT JOIN attachments ON attachments.id = messages.attachment_id \
             LEFT JOIN message_mentions AS mention ON mention.message_id = messages.id \
               AND mention.mentioned_user_id = $1 \
             WHERE room_memberships.user_id = $1 AND room_memberships.status = 'active' \
             AND messages.created_at >= COALESCE(room_memberships.joined_at, room_memberships.requested_at) \
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
