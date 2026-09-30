//! Sharing a favorite into a chat as a new message ("转发收藏"), split from `store.rs` (the
//! favorites CRUD) when that file crossed the 350-line gate.
use chrono::Utc;
use uuid::Uuid;

use crate::models::{StoredMessage, User};
use crate::state::{with_pool, AppState};

impl AppState {
    pub async fn forward_favorite(
        &self,
        favorite_id: Uuid,
        target_room_id: Uuid,
        forwarder: &User,
    ) -> Result<Option<StoredMessage>, sqlx::Error> {
        // TG-202: the full decision (restrictions; `message.post` in a channel), not only the
        // role grant the insert re-checks.
        if !self
            .has_chat_permission(target_room_id, forwarder.id, "message.send")
            .await?
        {
            return Ok(None);
        }
        // TG-204: a share lands in a forum's General topic; closed to the sender = refused.
        match self
            .resolve_post_topic(target_room_id, forwarder.id, None)
            .await
        {
            Err(crate::chats::TopicError::Database(error)) => return Err(error),
            Err(_) => return Ok(None),
            Ok(_) => {}
        }
        let id = Uuid::new_v4();
        let now = Utc::now();
        let display_name = self.resolve_display_name(target_room_id, forwarder).await;
        // TG-505: the original author's `forwards` rule, as for a direct message forward.
        let hidden = self
            .favorite_attribution_override(favorite_id, forwarder.id)
            .await?;
        let inserted = with_pool!(self, |pool| {
            sqlx::query(
                "INSERT INTO messages \
                 (id, room_id, sender_id, sender, content, attachment_id, favorite_id, \
                  forwarded_from_sender, forwarded_from_room_name, created_at) \
                 SELECT $1, $2, $3, $4, \
                   CASE WHEN favorites.kind = 'manual' AND favorites.content = '' \
                     THEN favorites.title ELSE favorites.content END, favorites.attachment_id, \
                   favorites.id, \
                   CASE WHEN favorites.kind = 'manual' THEN '我的收藏' \
                     ELSE COALESCE($7, favorites.source_sender) END, \
                   CASE WHEN favorites.kind = 'manual' THEN '个人收藏' \
                     WHEN $7 IS NOT NULL AND EXISTS (SELECT 1 FROM messages AS source_message \
                       JOIN direct_conversations AS direct \
                         ON direct.room_id = source_message.room_id \
                       WHERE source_message.id = favorites.source_message_id) THEN $7 \
                     ELSE favorites.source_room_name END, $6 \
                 FROM favorites WHERE favorites.id = $5 AND \
                   (favorites.user_id = $3 OR EXISTS \
                     (SELECT 1 FROM favorite_collaborators \
                     WHERE favorite_collaborators.favorite_id = favorites.id \
                        AND favorite_collaborators.user_id = $3) OR EXISTS \
                     (SELECT 1 FROM chat_pins \
                      JOIN messages AS pinned_message ON pinned_message.id = chat_pins.message_id \
                      JOIN chat_members AS pinned_membership ON pinned_membership.room_id = chat_pins.room_id \
                        AND pinned_membership.user_id = $3 AND pinned_membership.status = 'active' \
                      WHERE pinned_message.favorite_id = favorites.id)) \
                   AND EXISTS (SELECT 1 FROM chat_members \
                     JOIN chat_role_permissions ON chat_role_permissions.role_id = chat_members.role_id \
                     JOIN chats ON chats.id = chat_members.room_id AND chats.deleted_at IS NULL \
                     WHERE chat_members.room_id = $2 AND chat_members.user_id = $3 \
                       AND chat_members.status = 'active' \
                       AND chat_role_permissions.permission_key = 'message.send')",
            )
            .bind(id)
            .bind(target_room_id)
            .bind(forwarder.id)
            .bind(display_name)
            .bind(favorite_id)
            .bind(now)
            .bind(hidden)
            .execute(pool)
            .await
            .map(|result| result.rows_affected() > 0)
        })?;
        if !inserted {
            return Ok(None);
        }
        self.invalidate_message_cache(target_room_id).await;
        self.message_by_id(id, Some(forwarder.id)).await
    }
}
