//! Changing the capabilities that decide a chat's type: public username, forum topics and
//! slow mode. The one place a capability change and the supergroup upgrade it triggers are
//! written together, in one transaction.
//!
//! No HTTP surface here: setting a username, enabling a forum and configuring slow mode are
//! TG-206, TG-204 and TG-207. They call [`AppState::apply_chat_capabilities`] so the upgrade
//! rule lives in exactly one place. The member-limit trigger is applied by the membership
//! writes themselves (`chat_projection`).

use uuid::Uuid;

use super::chat_type::ChatType;
use super::supergroup_upgrade::{
    supergroup_upgrade_trigger, ChatCapabilityRequest, SupergroupUpgradeTrigger,
};
use crate::models::{Chat, ChatMessage};
use crate::state::{with_pool, AppState};

/// A requested change. `None` leaves a field as it is; `username: Some(None)` clears it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ChatCapabilityChange {
    pub username: Option<Option<String>>,
    pub is_forum: Option<bool>,
    pub slow_mode_seconds: Option<i64>,
}

#[derive(Debug)]
pub enum CapabilityError {
    NotFound,
    /// The chat's type (after any upgrade) cannot hold the requested capability, e.g. a
    /// forum in a channel or a username in a private chat.
    NotSupported(ChatType),
    Database(sqlx::Error),
}

impl From<sqlx::Error> for CapabilityError {
    fn from(error: sqlx::Error) -> Self {
        Self::Database(error)
    }
}

#[derive(Debug, Clone)]
pub struct CapabilityOutcome {
    pub chat: Chat,
    /// Set when this change turned a group into a supergroup.
    pub upgraded: Option<SupergroupUpgradeTrigger>,
}

type CapabilityRow = (String, i64, Option<String>, bool, i64);

impl AppState {
    pub async fn apply_chat_capabilities(
        &self,
        room_id: Uuid,
        change: ChatCapabilityChange,
    ) -> Result<CapabilityOutcome, CapabilityError> {
        let (chat_type, username, is_forum, slow_mode_seconds, upgraded) =
            with_pool!(self, |pool| {
                let mut transaction = pool.begin().await?;
                // Take the row lock first so two concurrent changes cannot both decide on a
                // stale type.
                sqlx::query("UPDATE chats SET chat_type = chat_type WHERE id = $1")
                    .bind(room_id)
                    .execute(&mut *transaction)
                    .await?;
                let row: Option<CapabilityRow> = sqlx::query_as(
                    "SELECT chat_type, CAST(member_count AS BIGINT), username, is_forum, \
                     CAST(slow_mode_seconds AS BIGINT) FROM chats \
                     WHERE id = $1 AND deleted_at IS NULL",
                )
                .bind(room_id)
                .fetch_optional(&mut *transaction)
                .await?;
                let Some((stored_type, member_count, username, is_forum, slow_mode)) = row else {
                    return Err(CapabilityError::NotFound);
                };
                let current: ChatType = stored_type.parse().unwrap_or_default();
                let username = change.username.clone().unwrap_or(username);
                let is_forum = change.is_forum.unwrap_or(is_forum);
                let slow_mode = change.slow_mode_seconds.unwrap_or(slow_mode).max(0);
                let requested = ChatCapabilityRequest {
                    member_count,
                    has_public_username: username.is_some(),
                    is_forum,
                    slow_mode_seconds: slow_mode,
                };
                let trigger = supergroup_upgrade_trigger(current, &requested);
                let target = trigger
                    .and_then(|trigger| current.upgraded(trigger))
                    .unwrap_or(current);
                if (username.is_some() && !target.allows_public_username())
                    || (is_forum && !target.allows_topics())
                    || (slow_mode > 0 && !target.allows_slow_mode())
                {
                    return Err(CapabilityError::NotSupported(target));
                }
                sqlx::query(
                    "UPDATE chats SET chat_type = $1, username = $2, is_forum = $3, \
                     slow_mode_seconds = $4 WHERE id = $5",
                )
                .bind(target.as_str())
                .bind(username.as_deref())
                .bind(is_forum)
                .bind(slow_mode)
                .bind(room_id)
                .execute(&mut *transaction)
                .await?;
                transaction.commit().await?;
                let upgraded = (target != current).then_some(trigger).flatten();
                Ok::<_, CapabilityError>((target, username, is_forum, slow_mode, upgraded))
            })?;
        let mut chat = self.chat(room_id).await.ok_or(CapabilityError::NotFound)?;
        chat.chat_type = chat_type;
        chat.username = username;
        chat.is_forum = is_forum;
        chat.slow_mode_seconds = slow_mode_seconds;
        self.cache_updated_chat(chat.clone()).await;
        if let Some(trigger) = upgraded {
            tracing::info!(%room_id, trigger = trigger.as_str(), "group upgraded to supergroup");
        }
        let mut announced = chat.clone();
        announced.membership_status = None;
        announced.membership_role = None;
        self.broadcast(room_id, ChatMessage::ChatUpdated { chat: announced })
            .await;
        Ok(CapabilityOutcome { chat, upgraded })
    }
}
