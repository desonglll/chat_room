//! Chat-scoped membership persistence and the queries that decorate a chat for a viewer.
//!
//! Provisioning a brand-new chat with its three system roles lives in `provisioning`.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use uuid::Uuid;

use super::chat_projection::upgrade_outgrown_group;
use crate::models::{Chat, ChatMembership, User};
use crate::state::{with_pool, AppState};

impl AppState {
    pub async fn membership_identity(
        &self,
        room_id: Uuid,
        user_id: Uuid,
    ) -> Result<Option<(String, String)>, sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query_as(
                "SELECT chat_members.status, chat_roles.name FROM chat_members \
             JOIN chat_roles ON chat_roles.id = chat_members.role_id \
             WHERE chat_members.room_id = $1 AND chat_members.user_id = $2",
            )
            .bind(room_id)
            .bind(user_id)
            .fetch_optional(pool)
            .await
        })
    }

    pub async fn decorate_chats_for_user(
        &self,
        chats: &mut [Chat],
        user_id: Uuid,
    ) -> Result<(), sqlx::Error> {
        // The chat's own projections (type, member count) are read from the row, not trusted
        // from the cache: a membership can change outside this module (an account deletion
        // cascades), and the database triggers keep the row right in every case.
        let rows: Vec<(Uuid, String, String, String, i64)> = with_pool!(self, |pool| {
            sqlx::query_as(
                "SELECT chat_members.room_id, chat_members.status, chat_roles.name, \
             chats.chat_type, CAST(chats.member_count AS BIGINT) \
             FROM chat_members JOIN chat_roles ON chat_roles.id = chat_members.role_id \
             JOIN chats ON chats.id = chat_members.room_id \
             WHERE chat_members.user_id = $1",
            )
            .bind(user_id)
            .fetch_all(pool)
            .await
        })?;
        let identities: HashMap<_, _> = rows
            .into_iter()
            .map(|(room_id, status, role, chat_type, member_count)| {
                (room_id, (status, role, chat_type, member_count))
            })
            .collect();
        for chat in chats.iter_mut() {
            // Always overwrite, never only conditionally set: `chats` may come
            // from the shared in-memory cache, which stores a chat's freshly
            // created state including the *creator's own* membership fields
            // (see handlers::create_chat) — a viewer with no real membership
            // must not inherit those stale values, or every chat would look
            // joined (and private chats would leak into everyone's listing).
            let identity = identities.get(&chat.id);
            chat.membership_status = identity.map(|(status, ..)| status.clone());
            chat.membership_role = identity.map(|(_, role, ..)| role.clone());
            if let Some((_, _, chat_type, member_count)) = identity {
                if let Ok(chat_type) = chat_type.parse() {
                    chat.chat_type = chat_type;
                }
                chat.member_count = *member_count;
            }
        }
        let unread: HashMap<_, _> = self
            .chat_unread_counts(user_id)
            .await?
            .into_iter()
            .collect();
        for chat in chats.iter_mut() {
            chat.unread_count = unread.get(&chat.id).copied().unwrap_or(0);
        }
        Ok(())
    }

    pub async fn chat_unread_counts(&self, user_id: Uuid) -> Result<Vec<(Uuid, i64)>, sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query_as(
            "SELECT memberships.room_id, COUNT(messages.id) AS unread_count \
             FROM chat_members AS memberships \
             LEFT JOIN chat_reads ON chat_reads.room_id = memberships.room_id \
               AND chat_reads.user_id = memberships.user_id \
             LEFT JOIN messages AS read_message ON read_message.id = chat_reads.message_id \
             LEFT JOIN messages ON messages.room_id = memberships.room_id \
               AND messages.recalled_at IS NULL \
               AND (messages.sender_id IS NULL OR messages.sender_id <> memberships.user_id) \
               AND (read_message.id IS NULL OR messages.created_at > read_message.created_at \
                 OR (messages.created_at = read_message.created_at AND messages.id > read_message.id)) \
             WHERE memberships.user_id = $1 AND memberships.status = 'active' \
             GROUP BY memberships.room_id ORDER BY memberships.room_id",
        )
        .bind(user_id)
        .fetch_all(pool)
        .await
        })
    }

    pub async fn account_membership_states(
        &self,
        user_id: Uuid,
    ) -> Result<Vec<(Uuid, String, String, i64, Option<DateTime<Utc>>)>, sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query_as(
                "SELECT memberships.room_id, memberships.status, roles.name, \
                 CAST(CASE WHEN review.role_id IS NULL THEN 0 \
                   ELSE (SELECT COUNT(*) FROM chat_members AS requests \
                     WHERE requests.room_id = memberships.room_id \
                       AND requests.status = 'pending') END AS BIGINT), \
                 CASE WHEN review.role_id IS NULL THEN NULL ELSE \
                   (SELECT MAX(requests.requested_at) FROM chat_members AS requests \
                     WHERE requests.room_id = memberships.room_id \
                       AND requests.status = 'pending') END \
                 FROM chat_members AS memberships \
                 JOIN chat_roles AS roles ON roles.id = memberships.role_id \
                 LEFT JOIN chat_role_permissions AS review ON review.role_id = roles.id \
                   AND review.permission_key = 'members.review' \
                 WHERE memberships.user_id = $1 ORDER BY memberships.room_id",
            )
            .bind(user_id)
            .fetch_all(pool)
            .await
        })
    }

    pub async fn request_chat_membership(
        &self,
        room_id: Uuid,
        user_id: Uuid,
        auto_activate: bool,
    ) -> Result<ChatMembership, sqlx::Error> {
        if self.chat_banned(room_id, user_id).await? {
            return Err(sqlx::Error::RowNotFound);
        }
        let now = Utc::now();
        let became_owner = with_pool!(self, |pool| {
            let mut transaction = pool.begin().await?;
            sqlx::query("UPDATE chats SET creator_user_id = creator_user_id WHERE id = $1")
                .bind(room_id)
                .execute(&mut *transaction)
                .await?;
            let has_active_member: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM chat_members \
                 WHERE room_id = $1 AND status = 'active')",
            )
            .bind(room_id)
            .fetch_one(&mut *transaction)
            .await?;
            let became_owner = !has_active_member;
            sqlx::query(
                "INSERT INTO chat_members \
             (room_id, user_id, role_id, status, requested_at, joined_at) \
             SELECT $1, $2, chat_roles.id, $3, $4, $5 FROM chat_roles \
             WHERE chat_roles.room_id = $6 AND chat_roles.name = $9 \
             ON CONFLICT(room_id, user_id) DO UPDATE SET \
               role_id = CASE WHEN $10 THEN excluded.role_id ELSE chat_members.role_id END, \
               status = CASE \
                 WHEN chat_members.status IN ('active', 'invited') OR $7 THEN 'active' \
                 ELSE 'pending' END, \
               joined_at = CASE \
                 WHEN chat_members.status IN ('active', 'invited') OR $8 \
                 THEN COALESCE(chat_members.joined_at, excluded.requested_at) \
                 ELSE chat_members.joined_at END, \
               requested_at = excluded.requested_at",
            )
            .bind(room_id)
            .bind(user_id)
            .bind(if auto_activate || became_owner {
                "active"
            } else {
                "pending"
            })
            .bind(now)
            .bind((auto_activate || became_owner).then_some(now))
            .bind(room_id)
            .bind(auto_activate || became_owner)
            .bind(auto_activate || became_owner)
            .bind(if became_owner { "owner" } else { "member" })
            .bind(became_owner)
            .execute(&mut *transaction)
            .await?;
            if became_owner {
                sqlx::query("UPDATE chats SET creator_user_id = $1 WHERE id = $2")
                    .bind(user_id)
                    .bind(room_id)
                    .execute(&mut *transaction)
                    .await?;
            }
            let upgraded = upgrade_outgrown_group!(&mut *transaction, room_id)?;
            transaction.commit().await?;
            Ok::<_, sqlx::Error>((became_owner, upgraded))
        })?;
        let (became_owner, upgraded) = became_owner;
        if became_owner {
            if let Some(mut chat) = self.chat(room_id).await {
                chat.creator_user_id = Some(user_id);
                self.cache_updated_chat(chat).await;
            }
        }
        self.sync_chat_projection(room_id, upgraded).await?;
        self.chat_membership(room_id, user_id)
            .await?
            .ok_or(sqlx::Error::RowNotFound)
    }

    pub async fn chat_membership(
        &self,
        room_id: Uuid,
        user_id: Uuid,
    ) -> Result<Option<ChatMembership>, sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query_as(
                "SELECT users.id AS user_id, users.username, users.avatar_emoji, \
             chat_members.nickname, \
             chat_roles.name AS role, chat_members.status, \
             chat_members.requested_at, chat_members.joined_at \
             FROM chat_members JOIN users ON users.id = chat_members.user_id \
             JOIN chat_roles ON chat_roles.id = chat_members.role_id \
             WHERE chat_members.room_id = $1 AND chat_members.user_id = $2",
            )
            .bind(room_id)
            .bind(user_id)
            .fetch_optional(pool)
            .await
        })
    }

    /// Set the caller's own chat nickname without a management permission.
    pub async fn set_own_nickname(
        &self,
        room_id: Uuid,
        user_id: Uuid,
        nickname: &str,
    ) -> Result<Option<ChatMembership>, sqlx::Error> {
        let changed = with_pool!(self, |pool| {
            sqlx::query(
                "UPDATE chat_members SET nickname = $1 \
                 WHERE room_id = $2 AND user_id = $3 AND status = 'active'",
            )
            .bind(nickname)
            .bind(room_id)
            .bind(user_id)
            .execute(pool)
            .await
            .map(|result| result.rows_affected())
        })?;
        if changed == 0 {
            return Ok(None);
        }
        self.chat_membership(room_id, user_id).await
    }

    /// Resolve and freeze nickname, display name, or username at message-send time.
    ///
    /// TG-202: a channel post is sent in the channel's name, so its frozen sender is the
    /// channel title; the author, when signatures are on, is `messages.post_author`.
    pub async fn resolve_display_name(&self, room_id: Uuid, user: &User) -> String {
        if let Some(chat) = self.chat(room_id).await {
            if chat.chat_type == super::ChatType::Channel {
                return chat.title;
            }
        }
        let nickname: Option<String> = with_pool!(self, |pool| {
            sqlx::query_scalar(
                "SELECT nickname FROM chat_members WHERE room_id = $1 AND user_id = $2",
            )
            .bind(room_id)
            .bind(user.id)
            .fetch_optional(pool)
            .await
        })
        .ok()
        .flatten();
        match nickname {
            Some(nickname) if !nickname.is_empty() => nickname,
            _ if !user.display_name.is_empty() => user.display_name.clone(),
            _ => user.username.clone(),
        }
    }

    pub async fn chat_members(&self, room_id: Uuid) -> Result<Vec<ChatMembership>, sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query_as(
            "SELECT users.id AS user_id, users.username, users.avatar_emoji, \
             chat_members.nickname, \
             chat_roles.name AS role, chat_members.status, \
             chat_members.requested_at, chat_members.joined_at \
             FROM chat_members JOIN users ON users.id = chat_members.user_id \
             JOIN chat_roles ON chat_roles.id = chat_members.role_id \
             WHERE chat_members.room_id = $1 \
             ORDER BY CASE chat_members.status WHEN 'pending' THEN 0 WHEN 'invited' THEN 1 ELSE 2 END, \
             LOWER(users.username)",
        )
        .bind(room_id)
        .fetch_all(pool)
        .await
        })
    }
}
