//! Chat bans are a governance projection separate from active membership.

use chrono::Utc;
use uuid::Uuid;

use crate::{
    models::ChatMembership,
    state::{with_pool, AppState},
};

impl AppState {
    pub async fn chat_banned(&self, room_id: Uuid, user_id: Uuid) -> Result<bool, sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM chat_bans WHERE room_id = $1 AND user_id = $2)",
            )
            .bind(room_id)
            .bind(user_id)
            .fetch_one(pool)
            .await
        })
    }

    pub async fn banned_chat_members(
        &self,
        room_id: Uuid,
    ) -> Result<Vec<ChatMembership>, sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query_as(
                "SELECT users.id AS user_id, users.username, users.avatar_emoji, \
                 '' AS nickname, 'member' AS role, 'banned' AS status, \
                 chat_bans.banned_at AS requested_at, \
                 NULLIF(chat_bans.banned_at, chat_bans.banned_at) AS joined_at \
                 FROM chat_bans JOIN users ON users.id = chat_bans.user_id \
                 WHERE chat_bans.room_id = $1 ORDER BY LOWER(users.username)",
            )
            .bind(room_id)
            .fetch_all(pool)
            .await
        })
    }

    pub async fn chat_ban_membership(
        &self,
        room_id: Uuid,
        user_id: Uuid,
    ) -> Result<Option<ChatMembership>, sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query_as(
                "SELECT users.id AS user_id, users.username, users.avatar_emoji, \
                 '' AS nickname, 'member' AS role, 'banned' AS status, \
                 chat_bans.banned_at AS requested_at, \
                 NULLIF(chat_bans.banned_at, chat_bans.banned_at) AS joined_at \
                 FROM chat_bans JOIN users ON users.id = chat_bans.user_id \
                 WHERE chat_bans.room_id = $1 AND chat_bans.user_id = $2",
            )
            .bind(room_id)
            .bind(user_id)
            .fetch_optional(pool)
            .await
        })
    }

    pub async fn ban_chat_member(
        &self,
        room_id: Uuid,
        user_id: Uuid,
        actor_id: Uuid,
    ) -> Result<Option<ChatMembership>, sqlx::Error> {
        let Some(mut membership) = self.chat_membership(room_id, user_id).await? else {
            return Ok(None);
        };
        with_pool!(self, |pool| {
            let mut transaction = pool.begin().await?;
            sqlx::query(
                "INSERT INTO chat_bans (room_id, user_id, banned_by, banned_at) \
                 VALUES ($1, $2, $3, $4) ON CONFLICT(room_id, user_id) DO UPDATE SET \
                 banned_by = excluded.banned_by, banned_at = excluded.banned_at",
            )
            .bind(room_id)
            .bind(user_id)
            .bind(actor_id)
            .bind(Utc::now())
            .execute(&mut *transaction)
            .await?;
            sqlx::query("DELETE FROM chat_reads WHERE room_id = $1 AND user_id = $2")
                .bind(room_id)
                .bind(user_id)
                .execute(&mut *transaction)
                .await?;
            sqlx::query("DELETE FROM chat_members WHERE room_id = $1 AND user_id = $2")
                .bind(room_id)
                .bind(user_id)
                .execute(&mut *transaction)
                .await?;
            transaction.commit().await
        })?;
        membership.status = "banned".into();
        Ok(Some(membership))
    }

    pub async fn unban_chat_member(
        &self,
        room_id: Uuid,
        user_id: Uuid,
    ) -> Result<Option<ChatMembership>, sqlx::Error> {
        let Some(membership) = self.chat_ban_membership(room_id, user_id).await? else {
            return Ok(None);
        };
        let removed = with_pool!(self, |pool| {
            sqlx::query("DELETE FROM chat_bans WHERE room_id = $1 AND user_id = $2")
                .bind(room_id)
                .bind(user_id)
                .execute(pool)
                .await
                .map(|result| result.rows_affected())
        })?;
        if removed == 0 {
            return Ok(None);
        }
        Ok(Some(membership))
    }
}
