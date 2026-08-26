//! Idempotent persistence for client-originated text messages.

use chrono::Utc;
use uuid::Uuid;

use super::store::{MessageRow, MESSAGE_SELECT};
use crate::models::StoredMessage;
use crate::state::{with_pool, AppState};

pub(crate) struct StoreMessageResult {
    pub message: StoredMessage,
    pub inserted: bool,
}

impl AppState {
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn store_message(
        &self,
        room_id: Uuid,
        sender_id: Uuid,
        sender: &str,
        sender_avatar: &str,
        content: &str,
        reply_to: Option<Uuid>,
        client_message_id: Option<Uuid>,
    ) -> Result<StoreMessageResult, sqlx::Error> {
        let id = Uuid::new_v4();
        let created_at = Utc::now();
        let reply_to = self.reply_preview(room_id, reply_to).await?;
        let inserted = with_pool!(self, |pool| {
            sqlx::query(
                "INSERT INTO messages \
                 (id, room_id, sender_id, sender, content, reply_to_id, client_message_id, created_at) \
                 SELECT $1, $2, $3, $4, $5, $6, $7, $8 \
                 WHERE EXISTS (SELECT 1 FROM room_memberships \
                   JOIN room_role_permissions ON room_role_permissions.role_id = room_memberships.role_id \
                   WHERE room_memberships.room_id = $2 AND room_memberships.user_id = $3 \
                     AND room_memberships.status = 'active' \
                     AND room_role_permissions.permission_key = 'message.send') \
                 ON CONFLICT (room_id, sender_id, client_message_id) \
                 WHERE client_message_id IS NOT NULL DO NOTHING",
            )
            .bind(id)
            .bind(room_id)
            .bind(sender_id)
            .bind(sender)
            .bind(content)
            .bind(reply_to.as_ref().map(|reply| reply.message_id))
            .bind(client_message_id)
            .bind(created_at)
            .execute(pool)
            .await
            .map(|result| result.rows_affected() > 0)
        })?;

        if !inserted {
            let query = format!(
                "{MESSAGE_SELECT} WHERE messages.room_id = $1 AND messages.sender_id = $2 \
                 AND messages.client_message_id = $3"
            );
            let row: MessageRow = with_pool!(self, |pool| {
                sqlx::query_as(&query)
                    .bind(room_id)
                    .bind(sender_id)
                    .bind(client_message_id)
                    .fetch_one(pool)
                    .await
            })?;
            let mut messages = vec![row.into_message(Some(sender_id))];
            self.attach_message_reactions(&mut messages).await?;
            return Ok(StoreMessageResult {
                message: messages.pop().expect("stored message exists"),
                inserted,
            });
        }

        self.invalidate_message_cache(room_id).await;

        Ok(StoreMessageResult {
            message: StoredMessage {
                id,
                client_message_id,
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
                favorite_id: None,
                forwarded_from: None,
                reactions: Vec::new(),
            },
            inserted,
        })
    }
}
