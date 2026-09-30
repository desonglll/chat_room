//! Invite-link persistence and the "who may manage links" decision.

use chrono::Utc;
use uuid::Uuid;

use super::{new_token, InviteLink, InviteLinkSettings};
use crate::chats::ChatAuthorization;
use crate::state::{with_pool, AppState};

/// The link-management right: `members.invite` held by an administrator, the owner, the
/// creator or a system administrator. `members.invite` is also a member-toggleable default
/// ("添加成员", direct adds); an ordinary member holding it may add people but, as in
/// Telegram, does not manage the chat's links.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LinkManager {
    /// May edit / revoke links other administrators created (`members.promote`).
    pub manages_others: bool,
    /// May approve / decline join requests (`members.review`).
    pub reviews: bool,
}

/// Why a management write did not happen.
#[derive(Debug, PartialEq, Eq)]
pub enum LinkWriteError {
    NotFound,
    /// Revoked links cannot be edited; live links cannot be deleted; the primary link is
    /// replaced, never edited.
    Conflict,
    Database(String),
}

impl From<sqlx::Error> for LinkWriteError {
    fn from(error: sqlx::Error) -> Self {
        LinkWriteError::Database(error.to_string())
    }
}

const SELECT_LINKS: &str = "SELECT links.id, links.room_id AS chat_id, links.token, links.title, \
     links.creator_user_id AS creator_id, \
     COALESCE(NULLIF(users.display_name, ''), users.username, '') AS creator_name, \
     links.expires_at, CAST(links.usage_limit AS BIGINT) AS usage_limit, \
     CAST(links.usage_count AS BIGINT) AS usage_count, links.requires_approval, \
     links.is_primary, links.revoked_at, links.created_at, \
     CAST((SELECT COUNT(*) FROM chat_members WHERE chat_members.invite_link_id = links.id \
       AND chat_members.status = 'pending') AS BIGINT) AS pending_count \
     FROM chat_invite_links AS links LEFT JOIN users ON users.id = links.creator_user_id";

impl AppState {
    /// `Some` when `user_id` may manage `room_id`'s invite links, decided now.
    pub async fn invite_link_manager(
        &self,
        room_id: Uuid,
        user_id: Uuid,
    ) -> Result<Option<LinkManager>, sqlx::Error> {
        let decision = self
            .authorize_chat_action(room_id, user_id, "members.invite")
            .await?;
        let manages = match decision {
            ChatAuthorization::SystemAdministrator | ChatAuthorization::Creator => true,
            ChatAuthorization::RolePermission => self
                .membership_identity(room_id, user_id)
                .await?
                .is_some_and(|(status, role)| {
                    status == "active" && matches!(role.as_str(), "owner" | "admin")
                }),
            _ => false,
        };
        if !manages {
            return Ok(None);
        }
        Ok(Some(LinkManager {
            manages_others: self
                .has_chat_permission(room_id, user_id, "members.promote")
                .await?,
            reviews: self
                .has_chat_permission(room_id, user_id, "members.review")
                .await?,
        }))
    }

    /// Every link of a chat, primary first, then live links newest first, then revoked ones.
    pub async fn chat_invite_links(&self, room_id: Uuid) -> Result<Vec<InviteLink>, sqlx::Error> {
        let sql = format!(
            "{SELECT_LINKS} WHERE links.room_id = $1 \
             ORDER BY CASE WHEN links.revoked_at IS NULL THEN 0 ELSE 1 END, \
               CASE WHEN links.is_primary THEN 0 ELSE 1 END, links.created_at DESC, links.id"
        );
        let now = Utc::now();
        let links: Vec<InviteLink> = with_pool!(self, |pool| {
            sqlx::query_as(&sql).bind(room_id).fetch_all(pool).await
        })?;
        Ok(links.into_iter().map(|link| link.at(now)).collect())
    }

    pub async fn chat_invite_link(
        &self,
        room_id: Uuid,
        link_id: Uuid,
    ) -> Result<Option<InviteLink>, sqlx::Error> {
        let sql = format!("{SELECT_LINKS} WHERE links.room_id = $1 AND links.id = $2");
        let link: Option<InviteLink> = with_pool!(self, |pool| {
            sqlx::query_as(&sql)
                .bind(room_id)
                .bind(link_id)
                .fetch_optional(pool)
                .await
        })?;
        Ok(link.map(|link| link.at(Utc::now())))
    }

    /// Give the chat a live primary link if it has none. Idempotent under concurrency: the
    /// partial unique index `chat_invite_links_primary_idx` lets exactly one insert win.
    pub async fn ensure_primary_invite_link(
        &self,
        room_id: Uuid,
        creator: Uuid,
    ) -> Result<(), sqlx::Error> {
        let exists: bool = with_pool!(self, |pool| {
            sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM chat_invite_links \
                 WHERE room_id = $1 AND is_primary AND revoked_at IS NULL)",
            )
            .bind(room_id)
            .fetch_one(pool)
            .await
        })?;
        if exists {
            return Ok(());
        }
        with_pool!(self, |pool| {
            sqlx::query(
                "INSERT INTO chat_invite_links \
                 (id, room_id, token, creator_user_id, created_at, is_primary) \
                 VALUES ($1, $2, $3, $4, $5, $6) ON CONFLICT DO NOTHING",
            )
            .bind(Uuid::new_v4())
            .bind(room_id)
            .bind(new_token())
            .bind(creator)
            .bind(Utc::now())
            .bind(true)
            .execute(pool)
            .await
            .map(|_| ())
        })
    }

    /// Revoke the live primary link (if any) and issue a new one, atomically.
    pub async fn replace_primary_invite_link(
        &self,
        room_id: Uuid,
        creator: Uuid,
    ) -> Result<InviteLink, sqlx::Error> {
        let id = Uuid::new_v4();
        let now = Utc::now();
        with_pool!(self, |pool| {
            let mut transaction = pool.begin().await?;
            sqlx::query(
                "UPDATE chat_invite_links SET revoked_at = $1 \
                 WHERE room_id = $2 AND is_primary AND revoked_at IS NULL",
            )
            .bind(now)
            .bind(room_id)
            .execute(&mut *transaction)
            .await?;
            sqlx::query(
                "INSERT INTO chat_invite_links \
                 (id, room_id, token, creator_user_id, created_at, is_primary) \
                 VALUES ($1, $2, $3, $4, $5, $6)",
            )
            .bind(id)
            .bind(room_id)
            .bind(new_token())
            .bind(creator)
            .bind(now)
            .bind(true)
            .execute(&mut *transaction)
            .await?;
            transaction.commit().await
        })?;
        self.chat_invite_link(room_id, id)
            .await?
            .ok_or(sqlx::Error::RowNotFound)
    }

    pub async fn create_invite_link(
        &self,
        room_id: Uuid,
        creator: Uuid,
        settings: &InviteLinkSettings,
    ) -> Result<InviteLink, sqlx::Error> {
        let id = Uuid::new_v4();
        with_pool!(self, |pool| {
            sqlx::query(
                "INSERT INTO chat_invite_links (id, room_id, token, creator_user_id, title, \
                 expires_at, usage_limit, requires_approval, is_primary, created_at) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
            )
            .bind(id)
            .bind(room_id)
            .bind(new_token())
            .bind(creator)
            .bind(&settings.title)
            .bind(settings.expires_at)
            .bind(settings.usage_limit)
            .bind(settings.requires_approval)
            .bind(false)
            .bind(Utc::now())
            .execute(pool)
            .await
            .map(|_| ())
        })?;
        self.chat_invite_link(room_id, id)
            .await?
            .ok_or(sqlx::Error::RowNotFound)
    }

    /// Replace a live additional link's settings. The token never changes.
    pub async fn edit_invite_link(
        &self,
        room_id: Uuid,
        link_id: Uuid,
        settings: &InviteLinkSettings,
    ) -> Result<InviteLink, LinkWriteError> {
        let changed = with_pool!(self, |pool| {
            sqlx::query(
                "UPDATE chat_invite_links SET title = $1, expires_at = $2, usage_limit = $3, \
                 requires_approval = $4 \
                 WHERE room_id = $5 AND id = $6 AND revoked_at IS NULL AND NOT is_primary",
            )
            .bind(&settings.title)
            .bind(settings.expires_at)
            .bind(settings.usage_limit)
            .bind(settings.requires_approval)
            .bind(room_id)
            .bind(link_id)
            .execute(pool)
            .await
            .map(|result| result.rows_affected())
        })?;
        let link = self
            .chat_invite_link(room_id, link_id)
            .await?
            .ok_or(LinkWriteError::NotFound)?;
        if changed == 0 {
            return Err(LinkWriteError::Conflict);
        }
        Ok(link)
    }

    /// Revoke a link, effective for the very next join. Revoking the primary link issues a
    /// new primary in the same transaction (Telegram's "revoke link"). Returns the revoked
    /// link; revoking an already revoked link is a no-op that returns it unchanged.
    pub async fn revoke_invite_link(
        &self,
        room_id: Uuid,
        link_id: Uuid,
        actor: Uuid,
    ) -> Result<InviteLink, LinkWriteError> {
        let link = self
            .chat_invite_link(room_id, link_id)
            .await?
            .ok_or(LinkWriteError::NotFound)?;
        if link.revoked_at.is_some() {
            return Ok(link);
        }
        if link.is_primary {
            self.replace_primary_invite_link(room_id, actor).await?;
        } else {
            with_pool!(self, |pool| {
                sqlx::query(
                    "UPDATE chat_invite_links SET revoked_at = $1 \
                     WHERE room_id = $2 AND id = $3 AND revoked_at IS NULL",
                )
                .bind(Utc::now())
                .bind(room_id)
                .bind(link_id)
                .execute(pool)
                .await
                .map(|_| ())
            })?;
        }
        self.chat_invite_link(room_id, link_id)
            .await?
            .ok_or(LinkWriteError::NotFound)
    }

    /// Delete a revoked link for good. Members it admitted stay; their row forgets the link.
    pub async fn delete_invite_link(
        &self,
        room_id: Uuid,
        link_id: Uuid,
    ) -> Result<(), LinkWriteError> {
        let link = self
            .chat_invite_link(room_id, link_id)
            .await?
            .ok_or(LinkWriteError::NotFound)?;
        if link.revoked_at.is_none() {
            return Err(LinkWriteError::Conflict);
        }
        with_pool!(self, |pool| {
            sqlx::query("DELETE FROM chat_invite_links WHERE room_id = $1 AND id = $2")
                .bind(room_id)
                .bind(link_id)
                .execute(pool)
                .await
                .map(|_| ())
        })?;
        Ok(())
    }
}
