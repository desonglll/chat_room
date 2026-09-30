//! The privacy checks other domains call on their read and write paths: avatar download,
//! forward attribution, group invitations, and voice messages.

use uuid::Uuid;

use super::{PrivacyKey, PrivacyTier};
use crate::state::{with_pool, AppState};

/// The attribution a forward carries when the original author's `forwards` rule does not
/// admit the forwarder. Telegram keeps the name and drops the account link; this product's
/// forwards carry only a name, so the name itself is what the rule withholds. Like the
/// existing `我的收藏` / `个人收藏` labels, it is stored data, not UI copy.
pub const HIDDEN_FORWARD_LABEL: &str = "隐藏的账号";

impl AppState {
    /// Whether `viewer` (None = an unauthenticated request) may download `owner`'s avatar
    /// image. An anonymous request is admitted only when no authenticated viewer could be
    /// refused — tier everybody, no deny exceptions, no blocks — because a refused viewer
    /// could otherwise simply drop their credentials.
    pub(crate) async fn profile_photo_visible(
        &self,
        owner: Uuid,
        viewer: Option<Uuid>,
    ) -> Result<bool, sqlx::Error> {
        if let Some(viewer) = viewer {
            return self
                .privacy_allows(owner, PrivacyKey::ProfilePhoto, viewer)
                .await;
        }
        if self.privacy_tier(owner, PrivacyKey::ProfilePhoto).await? != PrivacyTier::Everybody {
            return Ok(false);
        }
        let restricted: bool = with_pool!(self, |pool| {
            sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM user_privacy_exceptions \
                   WHERE user_id = $1 AND privacy_key = $2 AND effect = 'deny') \
                 OR EXISTS(SELECT 1 FROM user_blocks WHERE blocker_id = $1)",
            )
            .bind(owner)
            .bind(PrivacyKey::ProfilePhoto.as_str())
            .fetch_one(pool)
            .await
        })?;
        Ok(!restricted)
    }

    /// The attribution a forward of `source_message_id` by `forwarder` must carry instead of
    /// the author's name (TG-505 `forwards` rule), or `None` to keep the stored name. In a
    /// direct chat the source chat's name is the peer, i.e. the author, so callers apply the
    /// override to both. A forward of one's own message, or of a message without a known
    /// author, is never overridden.
    pub(crate) async fn forward_attribution_override(
        &self,
        source_message_id: Uuid,
        forwarder: Uuid,
    ) -> Result<Option<&'static str>, sqlx::Error> {
        let author: Option<Option<Uuid>> = with_pool!(self, |pool| {
            sqlx::query_scalar("SELECT sender_id FROM messages WHERE id = $1")
                .bind(source_message_id)
                .fetch_optional(pool)
                .await
        })?;
        self.author_hidden_from(author.flatten(), forwarder).await
    }

    /// [`Self::forward_attribution_override`] for a favorite snapshot: the author is the
    /// sender of the favorite's source message, when that message still exists.
    pub(crate) async fn favorite_attribution_override(
        &self,
        favorite_id: Uuid,
        forwarder: Uuid,
    ) -> Result<Option<&'static str>, sqlx::Error> {
        let author: Option<Option<Uuid>> = with_pool!(self, |pool| {
            sqlx::query_scalar(
                "SELECT messages.sender_id FROM favorites \
                 JOIN messages ON messages.id = favorites.source_message_id \
                 WHERE favorites.id = $1 AND favorites.kind <> 'manual'",
            )
            .bind(favorite_id)
            .fetch_optional(pool)
            .await
        })?;
        self.author_hidden_from(author.flatten(), forwarder).await
    }

    async fn author_hidden_from(
        &self,
        author: Option<Uuid>,
        forwarder: Uuid,
    ) -> Result<Option<&'static str>, sqlx::Error> {
        match author {
            Some(author) if author != forwarder => Ok((!self
                .privacy_allows(author, PrivacyKey::Forwards, forwarder)
                .await?)
                .then_some(HIDDEN_FORWARD_LABEL)),
            _ => Ok(None),
        }
    }

    /// Whether `inviter` may invite the account named `username` into a group. An unknown
    /// username is "allowed" here so the caller's own not-found handling stays the single
    /// answer for it.
    pub(crate) async fn group_invite_allowed(
        &self,
        inviter: Uuid,
        username: &str,
    ) -> Result<bool, sqlx::Error> {
        let invitee: Option<Uuid> = with_pool!(self, |pool| {
            sqlx::query_scalar("SELECT id FROM users WHERE LOWER(username) = LOWER($1)")
                .bind(username)
                .fetch_optional(pool)
                .await
        })?;
        match invitee {
            Some(invitee) => {
                self.privacy_allows(invitee, PrivacyKey::GroupInvites, inviter)
                    .await
            }
            None => Ok(true),
        }
    }

    /// Whether `sender` may send `recipient` a voice message in their private chat.
    /// Voice messages themselves land with TG-401, which must call this before accepting
    /// one; group chats are not governed by this rule (Telegram's behaviour).
    pub async fn voice_message_allowed(
        &self,
        sender: Uuid,
        recipient: Uuid,
    ) -> Result<bool, sqlx::Error> {
        self.privacy_allows(recipient, PrivacyKey::VoiceMessages, sender)
            .await
    }
}
