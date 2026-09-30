//! `private` chats: opening one between two friends, and presenting it to one participant.
//!
//! TG-208 moved this here from `direct_conversations`. A private chat is provisioned with the
//! same statements as a group (`provisioning`), its messages are read and written by the same
//! handlers as a group's, and every permission is decided by `authorize_chat_action` with the
//! `ChatType::Private` constraint as its last layer. What is private-specific lives in this
//! file and is limited to three things:
//!
//! * who may open one (accepted friends who have not blocked each other),
//! * that the pair shares at most one live chat (the `direct_conversations` index), and
//! * that each participant sees the chat titled after the *other* participant.

use chrono::Utc;
use uuid::Uuid;

use super::permissions::{member_role_grants, DEFAULT_MEMBER_PERMISSIONS};
use super::provisioning::{
    grant_role_permission, insert_chat_row, insert_system_role, system_role_id,
    upsert_active_member,
};
use super::ChatType;
use crate::direct_conversations::{self, PrivateChatPeer};
use crate::models::Chat;
use crate::social::canonical_pair;
use crate::state::{with_pool, AppState};

/// Why a private chat could not be opened.
#[derive(Debug)]
pub enum OpenPrivateChatError {
    /// The two accounts are not accepted friends, or one has blocked the other.
    NotAllowed,
    Database(sqlx::Error),
}

impl From<sqlx::Error> for OpenPrivateChatError {
    fn from(error: sqlx::Error) -> Self {
        Self::Database(error)
    }
}

impl PrivateChatPeer {
    /// The title this peer's private chat carries for the viewer: the viewer's remark, else
    /// the peer's display name, else the username. `/api/conversations` applies the same rule
    /// in SQL (`conversations::queries`).
    pub fn display_title(&self) -> &str {
        [&self.remark, &self.display_name]
            .into_iter()
            .find(|value| !value.is_empty())
            .unwrap_or(&self.username)
    }

    /// Rewrite the chat's display fields to how this viewer sees the peer. The stored title of
    /// a private chat is an internal `direct-<id>` placeholder that no client should show.
    pub fn present(&self, chat: &mut Chat) {
        chat.title = self.display_title().to_string();
        chat.avatar_emoji.clone_from(&self.avatar_emoji);
        chat.description.clone_from(&self.signature);
    }
}

impl AppState {
    /// Whether `chat_id` is a live `private` chat. Decided by `chat_type`, not by the index.
    pub async fn is_private_chat(&self, chat_id: Uuid) -> bool {
        self.chat(chat_id)
            .await
            .is_some_and(|chat| chat.chat_type == ChatType::Private)
    }

    /// Present every private chat in `chats` as `viewer_id` sees it. Other chats are untouched.
    pub async fn present_private_chats(
        &self,
        chats: &mut [Chat],
        viewer_id: Uuid,
    ) -> Result<(), sqlx::Error> {
        if !chats.iter().any(|chat| chat.chat_type == ChatType::Private) {
            return Ok(());
        }
        let peers = self.private_chat_peers(viewer_id).await?;
        for chat in chats
            .iter_mut()
            .filter(|chat| chat.chat_type == ChatType::Private)
        {
            if let Some(peer) = peers.get(&chat.id) {
                peer.present(chat);
            }
        }
        Ok(())
    }

    /// Return the private chat `user_id` shares with `peer_id`, creating it on first use.
    ///
    /// Idempotent and race-free: the friendship row is locked first, so two concurrent calls
    /// for the same pair serialise and the second one finds the first one's chat. Both
    /// participants end up active members holding the chat's `member` role.
    pub async fn open_private_chat(
        &self,
        user_id: Uuid,
        peer_id: Uuid,
    ) -> Result<Uuid, OpenPrivateChatError> {
        let (low, high) = canonical_pair(user_id, peer_id);
        let now = Utc::now();
        let mut created_chat = None;
        let chat_id = with_pool!(self, |pool| {
            let mut transaction = pool.begin().await?;
            let friendship_locked = sqlx::query(
                "UPDATE friendships SET updated_at = updated_at \
                 WHERE user_low_id = $1 AND user_high_id = $2 AND status = 'accepted'",
            )
            .bind(low)
            .bind(high)
            .execute(&mut *transaction)
            .await?
            .rows_affected();
            if friendship_locked == 0 {
                return Err(OpenPrivateChatError::NotAllowed);
            }
            let blocked: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM user_blocks WHERE \
                   (blocker_id = $1 AND blocked_id = $2) OR \
                   (blocker_id = $2 AND blocked_id = $1))",
            )
            .bind(low)
            .bind(high)
            .fetch_one(&mut *transaction)
            .await?;
            if blocked {
                return Err(OpenPrivateChatError::NotAllowed);
            }
            let chat_id = match direct_conversations::chat_for_pair(&mut *transaction, low, high)
                .await?
            {
                Some(chat_id) => chat_id,
                None => {
                    let chat = new_private_chat(now);
                    // The membership triggers count the two participants inserted below.
                    insert_chat_row(&mut *transaction, &chat, None).await?;
                    let role_id = system_role_id(chat.id, "member");
                    insert_system_role(&mut *transaction, &role_id, chat.id, "member", now).await?;
                    for permission in member_role_grants(DEFAULT_MEMBER_PERMISSIONS) {
                        grant_role_permission(&mut *transaction, &role_id, permission).await?;
                    }
                    direct_conversations::record_pair(&mut *transaction, chat.id, low, high, now)
                        .await?;
                    let chat_id = chat.id;
                    created_chat = Some(chat);
                    chat_id
                }
            };
            let role_id = system_role_id(chat_id, "member");
            for member_id in [low, high] {
                upsert_active_member(&mut *transaction, chat_id, member_id, &role_id, now).await?;
            }
            transaction.commit().await?;
            Ok::<_, OpenPrivateChatError>(chat_id)
        })?;
        if let Some(chat) = created_chat {
            self.cache_inserted_chat(chat).await;
        }
        Ok(chat_id)
    }
}

/// A brand-new private chat. Every field that differs from a group is spelled out: no
/// creator (neither participant owns it), `approval` join policy (nobody can join), and an
/// internal title that `PrivateChatPeer::present` always replaces before a client sees it.
fn new_private_chat(now: chrono::DateTime<Utc>) -> Chat {
    let id = Uuid::new_v4();
    Chat {
        id,
        chat_type: ChatType::Private,
        title: format!("direct-{}", id.simple()),
        creator_user_id: None,
        join_policy: "approval".into(),
        access_hash: Uuid::new_v4()
            .simple()
            .to_string()
            .chars()
            .take(16)
            .collect(),
        member_count: 2,
        created_at: now,
        ..Chat::default()
    }
}
