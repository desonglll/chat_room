//! Forwarding: copy a visible message into another chat as a new message.
//!
//! Split out of `store.rs` by TG-406 (that file sat at its size baseline): forwarding a poll
//! now also copies the poll inside the same transaction, which the single-statement version
//! could not do.

use chrono::Utc;
use uuid::Uuid;

use crate::models::{StoredMessage, User};
use crate::state::{with_pool, AppState};

impl AppState {
    /// Copy a still-visible message (and its attachment, if any) into another chat as
    /// a new message sent by `forwarder`. Returns `None` if the source message doesn't
    /// exist, isn't in `source_room_id`, or has been recalled.
    pub async fn forward_message(
        &self,
        source_message_id: Uuid,
        source_room_id: Uuid,
        target_room_id: Uuid,
        forwarder: &User,
    ) -> Result<Option<StoredMessage>, sqlx::Error> {
        self.forward_message_grouped(
            source_message_id,
            source_room_id,
            target_room_id,
            forwarder,
            None,
        )
        .await
    }

    /// TG-403: `forward_message` whose copy joins album `grouped_id` in the target
    /// (`messages::albums::ForwardPlan`); `None` is an ordinary forward.
    pub async fn forward_message_grouped(
        &self,
        source_message_id: Uuid,
        source_room_id: Uuid,
        target_room_id: Uuid,
        forwarder: &User,
        grouped_id: Option<Uuid>,
    ) -> Result<Option<StoredMessage>, sqlx::Error> {
        let id = Uuid::new_v4();
        let created_at = Utc::now();
        let forwarder_display_name = self.resolve_display_name(target_room_id, forwarder).await;
        // TG-505: a refused forward is credited to the hidden-account label, not the author.
        let attribution_override = self
            .forward_attribution_override(source_message_id, forwarder.id)
            .await?;
        let inserted = with_pool!(self, |pool| {
            async {
            let mut transaction = pool.begin().await?;
            let inserted = sqlx::query(
                "INSERT INTO messages \
                 (id, room_id, sender_id, sender, content, attachment_id, favorite_id, \
                  forwarded_from_sender, forwarded_from_room_name, created_at, grouped_id) \
                 SELECT $3, $4, $5, $6, source.content, source.attachment_id, source.favorite_id, COALESCE($8, source.sender), \
                   CASE WHEN direct.room_id IS NULL THEN source_chat.title \
                     ELSE COALESCE($8, NULLIF(peer.display_name, ''), peer.username) END, $7, $9 \
                 FROM messages AS source \
                 JOIN chats AS source_chat ON source_chat.id = source.room_id \
                   AND source_chat.deleted_at IS NULL \
                 LEFT JOIN direct_conversations AS direct ON direct.room_id = source_chat.id \
                 LEFT JOIN users AS peer ON peer.id = CASE \
                   WHEN direct.user_low_id = $5 THEN direct.user_high_id \
                   WHEN direct.user_high_id = $5 THEN direct.user_low_id ELSE NULL END \
                 WHERE source.id = $1 AND source.room_id = $2 AND source.recalled_at IS NULL \
                   AND EXISTS (SELECT 1 FROM chat_members AS source_membership \
                     JOIN chat_role_permissions AS source_permission \
                       ON source_permission.role_id = source_membership.role_id \
                     WHERE source_membership.room_id = $2 AND source_membership.user_id = $5 \
                       AND source_membership.status = 'active' \
                       AND source_permission.permission_key = 'message.send') \
                   AND EXISTS (SELECT 1 FROM chat_members AS target_membership \
                     JOIN chat_role_permissions AS target_permission \
                       ON target_permission.role_id = target_membership.role_id \
                     JOIN chats AS target_chat ON target_chat.id = target_membership.room_id \
                       AND target_chat.deleted_at IS NULL \
                     WHERE target_membership.room_id = $4 AND target_membership.user_id = $5 \
                       AND target_membership.status = 'active' \
                       AND target_permission.permission_key = 'message.send')",
            )
            .bind(source_message_id)
            .bind(source_room_id)
            .bind(id)
            .bind(target_room_id)
            .bind(forwarder.id)
            .bind(&forwarder_display_name)
            .bind(created_at)
            .bind(attribution_override)
            .bind(grouped_id)
            .execute(&mut *transaction)
            .await?
            .rows_affected()
                > 0;
            if inserted {
                // TG-406: a forwarded poll becomes a new poll (Telegram semantics), atomically.
                sqlx::query(crate::messages::polls::FORWARD_COPY_POLL)
                    .bind(id)
                    .bind(target_room_id)
                    .bind(forwarder.id)
                    .bind(created_at)
                    .bind(source_message_id)
                    .execute(&mut *transaction)
                    .await?;
                sqlx::query(crate::messages::polls::FORWARD_COPY_POLL_OPTIONS)
                    .bind(id)
                    .bind(source_message_id)
                    .execute(&mut *transaction)
                    .await?;
                // TG-401: a forwarded voice message stays a voice message (unlistened).
                sqlx::query(crate::attachments::voice::FORWARD_COPY_VOICE_NOTE)
                    .bind(id)
                    .bind(source_message_id)
                    .bind(created_at)
                    .execute(&mut *transaction)
                    .await?;
                sqlx::query(crate::attachments::voice::FORWARD_MARK_VOICE)
                    .bind(id)
                    .bind(source_message_id)
                    .execute(&mut *transaction)
                    .await?;
            }
            transaction.commit().await?;
            Ok::<_, sqlx::Error>(inserted)
            }
            .await
        })?;
        if !inserted {
            return Ok(None);
        }
        self.invalidate_message_cache(target_room_id).await;
        self.message_by_id(id, Some(forwarder.id)).await
    }
}
