//! Mutations applied to existing persisted messages.

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::state::AppState;

#[derive(Clone, Debug)]
pub(crate) struct RecallCursor {
    pub recalled_at: DateTime<Utc>,
    pub id: Uuid,
}

#[derive(Clone, Debug)]
pub(crate) struct EditCursor {
    pub edited_at: DateTime<Utc>,
    pub id: Uuid,
    pub content: String,
}

impl AppState {
    /// Replace a sender-owned message while retaining its identity and edit timestamp.
    pub async fn edit_message(
        &self,
        room_id: Uuid,
        sender_id: Uuid,
        message_id: Uuid,
        content: &str,
    ) -> Result<Option<DateTime<Utc>>, sqlx::Error> {
        let edited_at = Utc::now();
        let result = sqlx::query(
            "UPDATE messages SET content = ?, edited_at = ? \
             WHERE id = ? AND room_id = ? AND sender_id = ? AND recalled_at IS NULL",
        )
        .bind(content)
        .bind(edited_at)
        .bind(message_id)
        .bind(room_id)
        .bind(sender_id)
        .execute(self.pool())
        .await?;
        Ok((result.rows_affected() > 0).then_some(edited_at))
    }

    /// Mark a message as recalled while retaining its original database record.
    pub async fn recall_message(
        &self,
        room_id: Uuid,
        sender_id: Uuid,
        message_id: Uuid,
    ) -> Result<Option<DateTime<Utc>>, sqlx::Error> {
        let recalled_at = Utc::now();
        let attachment_id: Option<Option<Uuid>> = sqlx::query_scalar(
            "UPDATE messages SET recalled_at = ? \
             WHERE id = ? AND room_id = ? AND sender_id = ? AND recalled_at IS NULL \
             RETURNING attachment_id",
        )
        .bind(recalled_at)
        .bind(message_id)
        .bind(room_id)
        .bind(sender_id)
        .fetch_optional(self.pool())
        .await?;
        if let Some(Some(attachment_id)) = attachment_id {
            if let Err(error) = self.attachment_store().remove(attachment_id).await {
                tracing::warn!("remove recalled attachment failed: {error:#}");
            }
        }
        Ok(attachment_id.map(|_| recalled_at))
    }

    pub(crate) async fn latest_recall_cursor(
        &self,
        room_id: Uuid,
    ) -> Result<Option<RecallCursor>, sqlx::Error> {
        let row: Option<(DateTime<Utc>, Uuid)> = sqlx::query_as(
            "SELECT recalled_at, id FROM messages WHERE room_id = ? AND recalled_at IS NOT NULL \
             ORDER BY recalled_at DESC, id DESC LIMIT 1",
        )
        .bind(room_id)
        .fetch_optional(self.pool())
        .await?;
        Ok(row.map(|(recalled_at, id)| RecallCursor { recalled_at, id }))
    }

    pub(crate) async fn recalls_after(
        &self,
        room_id: Uuid,
        cursor: Option<&RecallCursor>,
        limit: i64,
    ) -> Result<Vec<RecallCursor>, sqlx::Error> {
        let limit = limit.clamp(1, 500);
        let rows: Vec<(DateTime<Utc>, Uuid)> = match cursor {
            Some(cursor) => sqlx::query_as(
                "SELECT recalled_at, id FROM messages WHERE room_id = ? AND recalled_at IS NOT NULL \
                 AND (recalled_at > ? OR (recalled_at = ? AND id > ?)) \
                 ORDER BY recalled_at ASC, id ASC LIMIT ?",
            )
            .bind(room_id)
            .bind(cursor.recalled_at)
            .bind(cursor.recalled_at)
            .bind(cursor.id)
            .bind(limit)
            .fetch_all(self.pool())
            .await?,
            None => sqlx::query_as(
                "SELECT recalled_at, id FROM messages WHERE room_id = ? AND recalled_at IS NOT NULL \
                 ORDER BY recalled_at ASC, id ASC LIMIT ?",
            )
            .bind(room_id)
            .bind(limit)
            .fetch_all(self.pool())
            .await?,
        };
        Ok(rows
            .into_iter()
            .map(|(recalled_at, id)| RecallCursor { recalled_at, id })
            .collect())
    }

    pub(crate) async fn latest_edit_cursor(
        &self,
        room_id: Uuid,
    ) -> Result<Option<EditCursor>, sqlx::Error> {
        let row: Option<(DateTime<Utc>, Uuid, String)> = sqlx::query_as(
            "SELECT edited_at, id, content FROM messages WHERE room_id = ? AND edited_at IS NOT NULL \
             ORDER BY edited_at DESC, id DESC LIMIT 1",
        )
        .bind(room_id)
        .fetch_optional(self.pool())
        .await?;
        Ok(row.map(|(edited_at, id, content)| EditCursor {
            edited_at,
            id,
            content,
        }))
    }

    pub(crate) async fn edits_after(
        &self,
        room_id: Uuid,
        cursor: Option<&EditCursor>,
        limit: i64,
    ) -> Result<Vec<EditCursor>, sqlx::Error> {
        let limit = limit.clamp(1, 500);
        let rows: Vec<(DateTime<Utc>, Uuid, String)> = match cursor {
            Some(cursor) => sqlx::query_as(
                "SELECT edited_at, id, content FROM messages WHERE room_id = ? AND edited_at IS NOT NULL \
                 AND (edited_at > ? OR (edited_at = ? AND id > ?)) \
                 ORDER BY edited_at ASC, id ASC LIMIT ?",
            )
            .bind(room_id)
            .bind(cursor.edited_at)
            .bind(cursor.edited_at)
            .bind(cursor.id)
            .bind(limit)
            .fetch_all(self.pool())
            .await?,
            None => sqlx::query_as(
                "SELECT edited_at, id, content FROM messages WHERE room_id = ? AND edited_at IS NOT NULL \
                 ORDER BY edited_at ASC, id ASC LIMIT ?",
            )
            .bind(room_id)
            .bind(limit)
            .fetch_all(self.pool())
            .await?,
        };
        Ok(rows
            .into_iter()
            .map(|(edited_at, id, content)| EditCursor {
                edited_at,
                id,
                content,
            })
            .collect())
    }
}
