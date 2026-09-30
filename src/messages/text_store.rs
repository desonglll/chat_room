//! Idempotent persistence for client-originated text messages.

use chrono::Utc;
use uuid::Uuid;

use super::reply_quotes::ReplyExtra;
use super::store::{MessageRow, MESSAGE_SELECT};
use crate::models::StoredMessage;
use crate::state::{with_pool, AppState};
use crate::stickers::custom_emoji::{entity_store::insert_message_entities, MessageEntity};

pub(crate) struct StoreMessageResult {
    pub message: StoredMessage,
    pub inserted: bool,
}

impl AppState {
    /// Store a text message with its TG-304 entities, in one transaction so the live poller
    /// never sees the text without them. `entities` must already be
    /// accepted (`AppState::accept_message_entities`).
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
        entities: &[MessageEntity],
        topic_id: Option<Uuid>,
        silent: bool,
        reply_extra: &ReplyExtra,
    ) -> Result<StoreMessageResult, sqlx::Error> {
        let id = Uuid::new_v4();
        let created_at = Utc::now();
        let reply_to = self.reply_preview(room_id, reply_to).await?;
        // TG-409: a cross-chat reply points at the source row (no same-chat preview exists);
        // its snapshot columns are what the target chat will read.
        let reply_to_id = reply_to
            .as_ref()
            .map(|reply| reply.message_id)
            .or(reply_extra.cross.as_ref().map(|cross| cross.message_id));
        let cross = reply_extra.cross.as_ref();
        let quote_text = reply_extra
            .quote
            .as_ref()
            .map(|quote| quote.text.clone())
            .or(cross.map(|cross| cross.snapshot.clone()));
        let quote_offset = reply_extra.quote.as_ref().map(|quote| quote.offset);
        let inserted = with_pool!(self, |pool| {
            async {
            let mut tx = pool.begin().await?;
            let inserted = sqlx::query(
                "INSERT INTO messages \
                 (id, room_id, sender_id, sender, content, reply_to_id, client_message_id, created_at, \
                  topic_id, silent, reply_quote_text, reply_quote_offset, reply_to_chat_id, \
                  reply_source_sender, reply_source_chat_title) \
                 SELECT $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15 \
                 WHERE EXISTS (SELECT 1 FROM chat_members \
                   JOIN chat_role_permissions ON chat_role_permissions.role_id = chat_members.role_id \
                   WHERE chat_members.room_id = $2 AND chat_members.user_id = $3 \
                     AND chat_members.status = 'active' \
                     AND chat_role_permissions.permission_key = 'message.send') \
                 ON CONFLICT (room_id, sender_id, client_message_id) \
                 WHERE client_message_id IS NOT NULL DO NOTHING",
            )
            .bind(id)
            .bind(room_id)
            .bind(sender_id)
            .bind(sender)
            .bind(content)
            .bind(reply_to_id)
            .bind(client_message_id)
            .bind(created_at)
            .bind(topic_id)
            .bind(silent)
            .bind(&quote_text)
            .bind(quote_offset)
            .bind(cross.map(|cross| cross.chat_id))
            .bind(cross.map(|cross| cross.sender.clone()))
            .bind(cross.map(|cross| cross.chat_title.clone()))
            .execute(&mut *tx)
            .await?
            .rows_affected()
                > 0;
            if inserted {
                insert_message_entities!(tx, id, entities)?;
            }
            tx.commit().await?;
            Ok::<bool, sqlx::Error>(inserted)
            }
            .await
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
                entities: entities.to_vec(),
                topic_id,
                silent,
                ..Default::default()
            },
            inserted,
        })
    }
}
