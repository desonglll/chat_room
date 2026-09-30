//! Channel broadcast semantics (TG-202): who holds which key in a channel, subscribing and
//! unsubscribing, and the signature switch.
//!
//! # Subscribers are `chat_members` rows without role machinery
//!
//! The card asks for a lightweight subscription instead of TG-201's member model. What makes
//! a 200 000-member chat heavy in this code base is not the membership row itself (TG-201's
//! keyset roster and statement-level `member_count` triggers are measured at 200k in
//! `member_page_perf_test`); it is everything a *join* fans out:
//!
//! - the `{user} joined the room` System frame and every `presence` frame carry the whole
//!   active roster (`chat_participants`) to every connection — O(N) per join, O(N²) overall;
//! - every `read` frame broadcasts a `read_receipt` to every connection;
//! - `auth_ok` loads every member's read cursor.
//!
//! So a subscriber keeps a `chat_members` row — it is what `can_read_chat`, the chat list,
//! unread counters, mute (`chat_members.muted_until`), notifications, search and the WebSocket
//! join already authorize on, and a second table would have to be taught to each of those
//! read gates, each a chance to leak — but the subscription is lightweight where it matters:
//!
//! - the subscriber role (`member`) holds **no key at all**: no rights rows, no restrictions,
//!   nothing to evaluate; reading is membership (`can_read_chat`);
//! - subscribing and unsubscribing broadcast nothing; `chat_participants` of a channel is the
//!   staff (owner and administrators), so `auth_ok`/`presence` stay O(staff);
//! - a channel sends no presence frames, no `read_receipt` frames, and `auth_ok` carries only
//!   the viewer's own read cursor;
//! - the subscriber count is `chats.member_count`, maintained by the TG-201 triggers inside
//!   the subscribing transaction.

use uuid::Uuid;

use super::permissions::{admin_role_grants, owner_role_grants, ADMIN_BASELINE};
use super::ChatType;
use crate::models::{ChatMembership, ReadReceipt};
use crate::state::{with_pool, AppState};

/// The rights of a channel administrator appointed without an explicit selection: Telegram's
/// channel-admin defaults (post, edit and delete posts, pin, change info, manage subscribers).
pub const CHANNEL_ADMIN_RIGHTS: &[&str] = &[
    "message.post",
    "message.edit_any",
    "message.delete_any",
    "message.pin",
    "chat.info",
    "room.settings",
    "members.review",
    "members.invite",
    "members.remove",
    "members.ban",
];

/// The grants of one system role of a freshly created chat of `chat_type`. Groups keep the
/// TG-201 sets (`member_grants` is the group's default permissions); a channel's subscriber
/// role holds nothing, so a subscriber can never pass even the inline SQL send guards.
pub fn system_role_grants(
    chat_type: ChatType,
    role: &str,
    member_grants: Vec<&'static str>,
) -> Vec<&'static str> {
    match (chat_type, role) {
        (_, "owner") => owner_role_grants(),
        (ChatType::Channel, "admin") => ADMIN_BASELINE
            .iter()
            .chain(CHANNEL_ADMIN_RIGHTS)
            .copied()
            .collect(),
        (ChatType::Channel, _) => Vec::new(),
        (_, "admin") => admin_role_grants(),
        _ => member_grants,
    }
}

/// What subscribing did.
#[derive(Debug, Clone)]
pub enum Subscription {
    /// The account now reads the channel.
    Active(ChatMembership),
    /// The channel approves subscribers; the request waits for `members.review`.
    Pending(ChatMembership),
}

#[derive(Debug)]
pub enum SubscriptionError {
    /// No such chat, or the chat is not a channel.
    NotAChannel,
    /// Banned from this channel.
    Banned,
    /// The owner cannot unsubscribe from their own channel.
    OwnerCannotLeave,
    Database(sqlx::Error),
}

impl From<sqlx::Error> for SubscriptionError {
    fn from(error: sqlx::Error) -> Self {
        SubscriptionError::Database(error)
    }
}

impl AppState {
    pub async fn is_channel(&self, room_id: Uuid) -> bool {
        self.chat(room_id)
            .await
            .is_some_and(|chat| chat.chat_type == ChatType::Channel)
    }

    /// Subscribe `user_id`. Idempotent: an active subscriber stays active. Broadcasts nothing
    /// (module docs); the subscriber count moves inside the membership transaction.
    pub async fn subscribe_channel(
        &self,
        room_id: Uuid,
        user_id: Uuid,
    ) -> Result<Subscription, SubscriptionError> {
        let chat = self
            .chat(room_id)
            .await
            .filter(|chat| chat.chat_type == ChatType::Channel)
            .ok_or(SubscriptionError::NotAChannel)?;
        if self.chat_banned(room_id, user_id).await? {
            return Err(SubscriptionError::Banned);
        }
        let membership = self
            .request_chat_membership(room_id, user_id, chat.join_policy == "open")
            .await?;
        Ok(if membership.status == "active" {
            Subscription::Active(membership)
        } else {
            Subscription::Pending(membership)
        })
    }

    /// Unsubscribe `user_id`. `Ok(false)` when there was nothing to leave.
    pub async fn unsubscribe_channel(
        &self,
        room_id: Uuid,
        user_id: Uuid,
    ) -> Result<bool, SubscriptionError> {
        if !self.is_channel(room_id).await {
            return Err(SubscriptionError::NotAChannel);
        }
        let Some(membership) = self.chat_membership(room_id, user_id).await? else {
            return Ok(false);
        };
        if membership.role == "owner" {
            return Err(SubscriptionError::OwnerCannotLeave);
        }
        let removed = self
            .delete_chat_membership(room_id, user_id, false)
            .await?
            .is_some();
        if removed {
            self.sync_chat_projection(room_id, false).await?;
            self.disconnect_chat_member(room_id, user_id, "membership left")
                .await;
        }
        Ok(removed)
    }

    /// Switch author signatures on or off. Returns the refreshed descriptor.
    pub async fn set_channel_signatures(
        &self,
        room_id: Uuid,
        enabled: bool,
    ) -> Result<Option<crate::models::Chat>, sqlx::Error> {
        let changed = with_pool!(self, |pool| {
            sqlx::query(
                "UPDATE chats SET signatures_enabled = $1 \
                 WHERE id = $2 AND chat_type = 'channel' AND deleted_at IS NULL",
            )
            .bind(enabled)
            .bind(room_id)
            .execute(pool)
            .await
            .map(|result| result.rows_affected())
        })?;
        if changed == 0 {
            return Ok(None);
        }
        let Some(mut chat) = self.chat(room_id).await else {
            return Ok(None);
        };
        chat.signatures_enabled = enabled;
        self.cache_updated_chat(chat.clone()).await;
        Ok(Some(chat))
    }

    /// A channel's `auth_ok` carries the viewer's own read cursor only: loading every
    /// subscriber's cursor per connection is O(subscribers).
    pub async fn own_read_receipts(
        &self,
        room_id: Uuid,
        user_id: Uuid,
    ) -> Result<Vec<ReadReceipt>, sqlx::Error> {
        let row: Option<(Uuid, String, Uuid)> = with_pool!(self, |pool| {
            sqlx::query_as(
                "SELECT chat_reads.user_id, users.username, chat_reads.message_id \
                 FROM chat_reads JOIN users ON users.id = chat_reads.user_id \
                 WHERE chat_reads.room_id = $1 AND chat_reads.user_id = $2",
            )
            .bind(room_id)
            .bind(user_id)
            .fetch_optional(pool)
            .await
        })?;
        Ok(row
            .map(|(user_id, username, message_id)| ReadReceipt {
                user_id,
                username,
                message_id,
            })
            .into_iter()
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chats::permissions::REGISTRY;

    #[test]
    fn channel_admin_rights_are_registered_keys() {
        for key in CHANNEL_ADMIN_RIGHTS {
            assert!(REGISTRY.iter().any(|entry| entry.key == *key), "{key}");
        }
    }

    #[test]
    fn a_channel_subscriber_holds_no_key_and_an_admin_may_post() {
        let member = system_role_grants(ChatType::Channel, "member", vec!["message.send"]);
        assert!(member.is_empty());
        let admin = system_role_grants(ChatType::Channel, "admin", Vec::new());
        assert!(admin.contains(&"message.post"));
        assert!(admin.contains(&"message.send_media"));
        assert!(admin.contains(&"message.edit_any"));
    }

    #[test]
    fn a_group_keeps_the_tg201_role_sets() {
        assert_eq!(
            system_role_grants(ChatType::Group, "member", vec!["message.send"]),
            vec!["message.send"]
        );
        assert_eq!(
            system_role_grants(ChatType::Group, "admin", Vec::new()),
            admin_role_grants()
        );
        assert!(!admin_role_grants().contains(&"message.post"));
    }
}
