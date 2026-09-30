//! Durable per-user chat read positions.

use chrono::Utc;
use sqlx::FromRow;
use uuid::Uuid;

use crate::models::ReadReceipt;
use crate::state::{with_pool, AppState};

impl AppState {
    /// Advance a user's chat read cursor only when the target message is newer.
    pub async fn store_read_cursor(
        &self,
        room_id: Uuid,
        user_id: Uuid,
        message_id: Uuid,
    ) -> Result<bool, sqlx::Error> {
        let changed = with_pool!(self, |pool| {
            sqlx::query(
                "INSERT INTO chat_reads (room_id, user_id, message_id, read_at) \
             SELECT $1, $2, $3, $4 WHERE EXISTS (\
                 SELECT 1 FROM messages WHERE id = $5 AND room_id = $6\
             ) ON CONFLICT(room_id, user_id) DO UPDATE SET \
                 message_id = excluded.message_id, read_at = excluded.read_at \
             WHERE EXISTS (\
                 SELECT 1 FROM messages AS next, messages AS current \
                 WHERE next.id = excluded.message_id \
                   AND current.id = chat_reads.message_id \
                   AND (next.created_at > current.created_at OR \
                        (next.created_at = current.created_at AND next.id > current.id))\
             )",
            )
            .bind(room_id)
            .bind(user_id)
            .bind(message_id)
            .bind(Utc::now())
            .bind(message_id)
            .bind(room_id)
            .execute(pool)
            .await
            .map(|result| result.rows_affected())
        })?;
        Ok(changed > 0)
    }

    pub async fn chat_read_receipts(&self, room_id: Uuid) -> Result<Vec<ReadReceipt>, sqlx::Error> {
        let rows: Vec<ReadRow> = with_pool!(self, |pool| {
            sqlx::query_as(
                "SELECT chat_reads.user_id, users.username, chat_reads.message_id \
             FROM chat_reads JOIN users ON users.id = chat_reads.user_id \
             WHERE chat_reads.room_id = $1 ORDER BY LOWER(users.username)",
            )
            .bind(room_id)
            .fetch_all(pool)
            .await
        })?;
        Ok(rows.into_iter().map(ReadRow::into_receipt).collect())
    }
}

#[derive(FromRow)]
struct ReadRow {
    user_id: Uuid,
    username: String,
    message_id: Uuid,
}

impl ReadRow {
    fn into_receipt(self) -> ReadReceipt {
        ReadReceipt {
            user_id: self.user_id,
            username: self.username,
            message_id: self.message_id,
        }
    }
}
