//! Resolving a link token, and joining a chat through it.
//!
//! The usage limit is enforced by one conditional statement inside the join transaction:
//!
//! ```sql
//! UPDATE chat_invite_links SET usage_count = usage_count + 1
//! WHERE id = $1 AND revoked_at IS NULL AND (expires_at IS NULL OR expires_at > $now)
//!   AND (usage_limit IS NULL OR usage_count < usage_limit)
//! ```
//!
//! PostgreSQL row-locks the link and re-evaluates the predicate after a concurrent writer
//! commits (READ COMMITTED's re-check), and SQLite serialises writers on the database lock,
//! so N concurrent joiners of a link limited to K admit exactly K. The membership upsert runs
//! in the same transaction; an account that turns out to be a member already rolls the
//! increment back, so re-opening a link never consumes it.

use chrono::{DateTime, Utc};
use uuid::Uuid;

use super::{link_state, InviteLinkState, InvitePreview};
use crate::admin_system_lock::chat_lock_reason;
use crate::chats::chat_projection::upgrade_outgrown_group;
use crate::models::Chat;
use crate::state::{with_pool, AppState};

/// A link row as the join path needs it.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ResolvedLink {
    pub id: Uuid,
    pub room_id: Uuid,
    pub creator_user_id: Option<Uuid>,
    pub expires_at: Option<DateTime<Utc>>,
    pub usage_limit: Option<i64>,
    pub usage_count: i64,
    pub requires_approval: bool,
    pub is_primary: bool,
    pub revoked_at: Option<DateTime<Utc>>,
}

/// Why a link cannot be used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InviteRejection {
    /// Unknown token, or its chat is gone or private.
    NotFound,
    Unusable(InviteLinkState),
    Banned,
    /// A system administrator locked the chat (backup restore, incident).
    Locked,
}

/// What a successful use of a link did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InviteJoinOutcome {
    Joined { chat_id: Uuid },
    AlreadyMember { chat_id: Uuid },
    Requested { chat_id: Uuid },
}

#[derive(Debug)]
pub enum InviteJoinError {
    Rejected(InviteRejection),
    Database(sqlx::Error),
}

impl From<sqlx::Error> for InviteJoinError {
    fn from(error: sqlx::Error) -> Self {
        InviteJoinError::Database(error)
    }
}

impl From<InviteRejection> for InviteJoinError {
    fn from(rejection: InviteRejection) -> Self {
        InviteJoinError::Rejected(rejection)
    }
}

/// Inside the join transaction: consumed a use, found the account already a member, or
/// found the link no longer admits anyone.
enum Admission {
    Admitted { upgraded: bool },
    AlreadyMember,
    Refused,
}

impl AppState {
    async fn resolve_invite_token(&self, token: &str) -> Result<Option<ResolvedLink>, sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query_as(
                "SELECT id, room_id, creator_user_id, expires_at, \
                 CAST(usage_limit AS BIGINT) AS usage_limit, \
                 CAST(usage_count AS BIGINT) AS usage_count, requires_approval, is_primary, \
                 revoked_at FROM chat_invite_links WHERE token = $1",
            )
            .bind(token)
            .fetch_optional(pool)
            .await
        })
    }

    /// The link and its chat, if the link may be used right now. An additional link dies with
    /// its creator's right to manage links (read-time re-authorization): an administrator who
    /// is dismissed, or leaves, takes the links they issued with them. The primary link is
    /// the chat's own and survives its creator.
    async fn usable_invite_link(
        &self,
        token: &str,
    ) -> Result<(ResolvedLink, Chat), InviteJoinError> {
        let link = self
            .resolve_invite_token(token)
            .await?
            .ok_or(InviteRejection::NotFound)?;
        let chat = self
            .chat(link.room_id)
            .await
            .ok_or(InviteRejection::NotFound)?;
        if self.is_private_chat(link.room_id).await {
            return Err(InviteRejection::NotFound.into());
        }
        let state = link_state(
            link.revoked_at,
            link.expires_at,
            link.usage_limit,
            link.usage_count,
            Utc::now(),
        );
        if state != InviteLinkState::Active {
            return Err(InviteRejection::Unusable(state).into());
        }
        if !link.is_primary {
            let creator_manages = match link.creator_user_id {
                Some(creator) => self
                    .invite_link_manager(link.room_id, creator)
                    .await?
                    .is_some(),
                None => false,
            };
            if !creator_manages {
                return Err(InviteRejection::Unusable(InviteLinkState::Revoked).into());
            }
        }
        Ok((link, chat))
    }

    /// What `viewer` may see about the chat behind `token` before joining.
    pub async fn invite_preview(
        &self,
        token: &str,
        viewer: Uuid,
    ) -> Result<InvitePreview, InviteJoinError> {
        let (link, chat) = self.usable_invite_link(token).await?;
        let membership_status = self
            .membership_identity(link.room_id, viewer)
            .await?
            .map(|(status, _)| status);
        let member_count: i64 = with_pool!(self, |pool| {
            sqlx::query_scalar("SELECT CAST(member_count AS BIGINT) FROM chats WHERE id = $1")
                .bind(link.room_id)
                .fetch_one(pool)
                .await
        })?;
        Ok(InvitePreview {
            chat_id: (membership_status.as_deref() == Some("active")).then_some(chat.id),
            title: chat.title,
            description: chat.description,
            avatar_emoji: chat.avatar_emoji,
            chat_type: chat.chat_type,
            member_count,
            requires_approval: link.requires_approval,
            membership_status,
        })
    }

    /// Use `token` as `user_id`. A ban always wins; an approval link queues the account in
    /// the existing join-request queue; any other link admits it and consumes one use.
    pub async fn join_by_invite_link(
        &self,
        token: &str,
        user_id: Uuid,
    ) -> Result<InviteJoinOutcome, InviteJoinError> {
        let (link, _) = self.usable_invite_link(token).await?;
        let room_id = link.room_id;
        if chat_lock_reason(self, room_id).await?.is_some() {
            return Err(InviteRejection::Locked.into());
        }
        if self.chat_banned(room_id, user_id).await? {
            return Err(InviteRejection::Banned.into());
        }
        let previous = self.membership_identity(room_id, user_id).await?;
        let previous_status = previous.as_ref().map(|(status, _)| status.as_str());
        if previous_status == Some("active") {
            return Ok(InviteJoinOutcome::AlreadyMember { chat_id: room_id });
        }
        // An account an administrator already invited directly needs no second approval.
        if link.requires_approval && previous_status != Some("invited") {
            self.queue_invite_link_request(&link, user_id).await?;
            return Ok(InviteJoinOutcome::Requested { chat_id: room_id });
        }
        match self.admit_through_link(&link, user_id).await? {
            Admission::Admitted { upgraded } => {
                self.sync_chat_projection(room_id, upgraded).await?;
                Ok(InviteJoinOutcome::Joined { chat_id: room_id })
            }
            Admission::AlreadyMember => Ok(InviteJoinOutcome::AlreadyMember { chat_id: room_id }),
            Admission::Refused => {
                // Lost the race for the last use, or revoked/expired in between: say which.
                let current = self
                    .resolve_invite_token(token)
                    .await?
                    .ok_or(InviteRejection::NotFound)?;
                let state = link_state(
                    current.revoked_at,
                    current.expires_at,
                    current.usage_limit,
                    current.usage_count,
                    Utc::now(),
                );
                Err(InviteRejection::Unusable(match state {
                    InviteLinkState::Active => InviteLinkState::LimitReached,
                    other => other,
                })
                .into())
            }
        }
    }

    async fn admit_through_link(
        &self,
        link: &ResolvedLink,
        user_id: Uuid,
    ) -> Result<Admission, sqlx::Error> {
        let now = Utc::now();
        with_pool!(self, |pool| {
            let mut transaction = pool.begin().await?;
            let consumed = sqlx::query(
                "UPDATE chat_invite_links SET usage_count = usage_count + 1 \
                 WHERE id = $1 AND revoked_at IS NULL \
                   AND (expires_at IS NULL OR expires_at > $2) \
                   AND (usage_limit IS NULL OR usage_count < usage_limit)",
            )
            .bind(link.id)
            .bind(now)
            .execute(&mut *transaction)
            .await?
            .rows_affected();
            if consumed == 0 {
                return Ok(Admission::Refused);
            }
            let admitted = sqlx::query(
                "INSERT INTO chat_members \
                 (room_id, user_id, role_id, status, requested_at, joined_at, invite_link_id) \
                 SELECT $1, $2, chat_roles.id, 'active', $3, $3, $4 FROM chat_roles \
                 WHERE chat_roles.room_id = $1 AND chat_roles.name = 'member' \
                 ON CONFLICT(room_id, user_id) DO UPDATE SET status = 'active', \
                   joined_at = COALESCE(chat_members.joined_at, excluded.joined_at), \
                   invite_link_id = excluded.invite_link_id \
                 WHERE chat_members.status <> 'active'",
            )
            .bind(link.room_id)
            .bind(user_id)
            .bind(now)
            .bind(link.id)
            .execute(&mut *transaction)
            .await?
            .rows_affected();
            if admitted == 0 {
                // Already active (a concurrent join by the same account): give the use back.
                transaction.rollback().await?;
                return Ok(Admission::AlreadyMember);
            }
            let upgraded = upgrade_outgrown_group!(&mut *transaction, link.room_id)?;
            transaction.commit().await?;
            Ok(Admission::Admitted { upgraded })
        })
    }

    /// Put the account in the join-request queue, remembering the link it came through.
    async fn queue_invite_link_request(
        &self,
        link: &ResolvedLink,
        user_id: Uuid,
    ) -> Result<(), sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query(
                "INSERT INTO chat_members \
                 (room_id, user_id, role_id, status, requested_at, invite_link_id) \
                 SELECT $1, $2, chat_roles.id, 'pending', $3, $4 FROM chat_roles \
                 WHERE chat_roles.room_id = $1 AND chat_roles.name = 'member' \
                 ON CONFLICT(room_id, user_id) DO UPDATE SET status = 'pending', \
                   requested_at = excluded.requested_at, \
                   invite_link_id = excluded.invite_link_id \
                 WHERE chat_members.status = 'pending'",
            )
            .bind(link.room_id)
            .bind(user_id)
            .bind(Utc::now())
            .bind(link.id)
            .execute(pool)
            .await
            .map(|_| ())
        })
    }
}
