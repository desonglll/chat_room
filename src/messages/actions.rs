//! Mutations applied to existing persisted messages.

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::state::{with_pool, AppState};

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
    /// Also un-recalls the message: a sender can always re-edit their own recalled
    /// draft (only they could see it), and doing so republishes it to everyone.
    pub async fn edit_message(
        &self,
        room_id: Uuid,
        sender_id: Uuid,
        message_id: Uuid,
        content: &str,
    ) -> Result<Option<DateTime<Utc>>, sqlx::Error> {
        let edited_at = Utc::now();
        let attachment_id: Option<Option<Uuid>> = with_pool!(self, |pool| {
            sqlx::query_scalar(
                "UPDATE messages SET content = $1, edited_at = $2, recalled_at = NULL \
             WHERE id = $3 AND room_id = $4 AND sender_id = $5 \
             RETURNING attachment_id",
            )
            .bind(content)
            .bind(edited_at)
            .bind(message_id)
            .bind(room_id)
            .bind(sender_id)
            .fetch_optional(pool)
            .await
        })?;
        // A re-edit un-recalls the message, which can resurrect a reference to
        // a physical file that was marked orphaned while it was recalled.
        if let Some(Some(attachment_id)) = attachment_id {
            if let Err(error) = self.recompute_attachment_orphan_status(attachment_id).await {
                tracing::warn!("recompute attachment orphan status failed: {error:#}");
            }
        }
        Ok(attachment_id.map(|_| edited_at))
    }

    /// Mark a message as recalled while retaining its original database record.
    /// Never deletes the attachment's physical file directly — a forwarded
    /// copy of the same message may still reference it. Instead this
    /// recomputes and marks (or clears) `orphaned_at` for whatever physical
    /// file it's backed by, for a future cleanup job to act on.
    pub async fn recall_message(
        &self,
        room_id: Uuid,
        sender_id: Uuid,
        message_id: Uuid,
    ) -> Result<Option<DateTime<Utc>>, sqlx::Error> {
        let recalled_at = Utc::now();
        let attachment_id: Option<Option<Uuid>> = with_pool!(self, |pool| {
            sqlx::query_scalar(
                "UPDATE messages SET recalled_at = $1 \
             WHERE id = $2 AND room_id = $3 AND sender_id = $4 AND recalled_at IS NULL \
             RETURNING attachment_id",
            )
            .bind(recalled_at)
            .bind(message_id)
            .bind(room_id)
            .bind(sender_id)
            .fetch_optional(pool)
            .await
        })?;
        if let Some(Some(attachment_id)) = attachment_id {
            if let Err(error) = self.recompute_attachment_orphan_status(attachment_id).await {
                tracing::warn!("recompute attachment orphan status failed: {error:#}");
            }
        }
        Ok(attachment_id.map(|_| recalled_at))
    }

    pub(crate) async fn latest_recall_cursor(
        &self,
        room_id: Uuid,
    ) -> Result<Option<RecallCursor>, sqlx::Error> {
        let row: Option<(DateTime<Utc>, Uuid)> = with_pool!(self, |pool| {
            sqlx::query_as(
            "SELECT recalled_at, id FROM messages WHERE room_id = $1 AND recalled_at IS NOT NULL \
             ORDER BY recalled_at DESC, id DESC LIMIT 1",
        )
        .bind(room_id)
        .fetch_optional(pool)
        .await
        })?;
        Ok(row.map(|(recalled_at, id)| RecallCursor { recalled_at, id }))
    }

    pub(crate) async fn recalls_after(
        &self,
        room_id: Uuid,
        cursor: Option<&RecallCursor>,
        limit: i64,
    ) -> Result<Vec<RecallCursor>, sqlx::Error> {
        let limit = limit.clamp(1, 500);
        let rows: Vec<(DateTime<Utc>, Uuid)> = with_pool!(self, |pool| {
            let rows = match cursor {
            Some(cursor) => sqlx::query_as(
                "SELECT recalled_at, id FROM messages WHERE room_id = $1 AND recalled_at IS NOT NULL \
                 AND (recalled_at > $2 OR (recalled_at = $3 AND id > $4)) \
                 ORDER BY recalled_at ASC, id ASC LIMIT $5",
            )
            .bind(room_id)
            .bind(cursor.recalled_at)
            .bind(cursor.recalled_at)
            .bind(cursor.id)
            .bind(limit)
            .fetch_all(pool)
            .await?,
            None => sqlx::query_as(
                "SELECT recalled_at, id FROM messages WHERE room_id = $1 AND recalled_at IS NOT NULL \
                 ORDER BY recalled_at ASC, id ASC LIMIT $2",
            )
            .bind(room_id)
            .bind(limit)
            .fetch_all(pool)
            .await?,
            };
            Ok::<_, sqlx::Error>(rows)
        })?;
        Ok(rows
            .into_iter()
            .map(|(recalled_at, id)| RecallCursor { recalled_at, id })
            .collect())
    }

    pub(crate) async fn latest_edit_cursor(
        &self,
        room_id: Uuid,
    ) -> Result<Option<EditCursor>, sqlx::Error> {
        let row: Option<(DateTime<Utc>, Uuid, String)> = with_pool!(self, |pool| {
            sqlx::query_as(
            "SELECT edited_at, id, content FROM messages WHERE room_id = $1 AND edited_at IS NOT NULL \
             ORDER BY edited_at DESC, id DESC LIMIT 1",
        )
        .bind(room_id)
        .fetch_optional(pool)
        .await
        })?;
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
        let rows: Vec<(DateTime<Utc>, Uuid, String)> = with_pool!(self, |pool| {
            let rows = match cursor {
            Some(cursor) => sqlx::query_as(
                "SELECT edited_at, id, content FROM messages WHERE room_id = $1 AND edited_at IS NOT NULL \
                 AND (edited_at > $2 OR (edited_at = $3 AND id > $4)) \
                 ORDER BY edited_at ASC, id ASC LIMIT $5",
            )
            .bind(room_id)
            .bind(cursor.edited_at)
            .bind(cursor.edited_at)
            .bind(cursor.id)
            .bind(limit)
            .fetch_all(pool)
            .await?,
            None => sqlx::query_as(
                "SELECT edited_at, id, content FROM messages WHERE room_id = $1 AND edited_at IS NOT NULL \
                 ORDER BY edited_at ASC, id ASC LIMIT $2",
            )
            .bind(room_id)
            .bind(limit)
            .fetch_all(pool)
            .await?,
            };
            Ok::<_, sqlx::Error>(rows)
        })?;
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
