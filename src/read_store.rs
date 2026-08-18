//! Durable per-user room read positions.

use chrono::Utc;
use sqlx::FromRow;
use uuid::Uuid;

use crate::models::ReadReceipt;
use crate::state::AppState;

impl AppState {
    /// Advance a user's room read cursor only when the target message is newer.
    pub async fn store_read_cursor(
        &self,
        room_id: Uuid,
        user_id: Uuid,
        message_id: Uuid,
    ) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(
            "INSERT INTO room_reads (room_id, user_id, message_id, read_at) \
             SELECT ?, ?, ?, ? WHERE EXISTS (\
                 SELECT 1 FROM messages WHERE id = ? AND room_id = ?\
             ) ON CONFLICT(room_id, user_id) DO UPDATE SET \
                 message_id = excluded.message_id, read_at = excluded.read_at \
             WHERE EXISTS (\
                 SELECT 1 FROM messages AS next, messages AS current \
                 WHERE next.id = excluded.message_id \
                   AND current.id = room_reads.message_id \
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
        .execute(self.pool())
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn room_read_receipts(&self, room_id: Uuid) -> Result<Vec<ReadReceipt>, sqlx::Error> {
        let rows: Vec<ReadRow> = sqlx::query_as(
            "SELECT room_reads.user_id, users.username, room_reads.message_id \
             FROM room_reads JOIN users ON users.id = room_reads.user_id \
             WHERE room_reads.room_id = ? ORDER BY users.username COLLATE NOCASE",
        )
        .bind(room_id)
        .fetch_all(self.pool())
        .await?;
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
