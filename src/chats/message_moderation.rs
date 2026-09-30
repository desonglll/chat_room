//! Who may change an existing message, and who may react to one (TG-202).
//!
//! TG-201 registered `message.edit_any`, `message.delete_any`, `message.edit_own` and
//! `message.recall_own` without a caller. The WebSocket `edit` / `recall` frames now decide
//! here: one's own message needs the `_own` key (editing also needs the right to send, as
//! before), someone else's needs the `_any` key — which is how a channel administrator edits
//! or deletes another administrator's post, and how a group administrator deletes a member's
//! message.

use uuid::Uuid;

use super::ChatType;
use crate::state::{with_pool, AppState};

/// A change to an existing message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageChange {
    Edit,
    Recall,
}

/// Whose message the authorized change applies to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageScope {
    Own,
    Any,
}

impl MessageChange {
    const fn own_key(self) -> &'static str {
        match self {
            MessageChange::Edit => "message.edit_own",
            MessageChange::Recall => "message.recall_own",
        }
    }

    const fn any_key(self) -> &'static str {
        match self {
            MessageChange::Edit => "message.edit_any",
            MessageChange::Recall => "message.delete_any",
        }
    }
}

impl AppState {
    /// `None` when the message is not in this chat or `user_id` may not make the change.
    pub async fn authorize_message_change(
        &self,
        room_id: Uuid,
        user_id: Uuid,
        message_id: Uuid,
        change: MessageChange,
    ) -> Result<Option<MessageScope>, sqlx::Error> {
        let sender: Option<Option<Uuid>> = with_pool!(self, |pool| {
            sqlx::query_scalar("SELECT sender_id FROM messages WHERE id = $1 AND room_id = $2")
                .bind(message_id)
                .bind(room_id)
                .fetch_optional(pool)
                .await
        })?;
        let Some(sender) = sender else {
            return Ok(None);
        };
        if sender == Some(user_id) {
            let may_send = change == MessageChange::Recall
                || self
                    .has_chat_permission(room_id, user_id, "message.send")
                    .await?;
            let allowed = may_send
                && self
                    .has_chat_permission(room_id, user_id, change.own_key())
                    .await?;
            return Ok(allowed.then_some(MessageScope::Own));
        }
        let allowed = self
            .has_chat_permission(room_id, user_id, change.any_key())
            .await?;
        Ok(allowed.then_some(MessageScope::Any))
    }

    /// Reacting: in a channel every subscriber may react (a subscriber holds no key, reading
    /// is membership); elsewhere reacting is a form of sending.
    pub async fn may_react(&self, room_id: Uuid, user_id: Uuid) -> Result<bool, sqlx::Error> {
        if self.chat_type(room_id).await? == Some(ChatType::Channel) {
            return self.can_read_chat(room_id, user_id).await;
        }
        self.has_chat_permission(room_id, user_id, "message.send")
            .await
    }
}
