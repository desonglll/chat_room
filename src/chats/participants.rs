//! Durable chat participant roster used by group read receipts.

use sqlx::FromRow;
use uuid::Uuid;

use crate::models::ChatMember;
use crate::state::{with_pool, AppState};

impl AppState {
    pub async fn chat_participants(&self, room_id: Uuid) -> Result<Vec<ChatMember>, sqlx::Error> {
        let rows: Vec<ParticipantRow> = with_pool!(self, |pool| {
            sqlx::query_as(
                "SELECT users.id AS user_id, users.username, users.avatar_emoji \
             FROM chat_members JOIN users ON users.id = chat_members.user_id \
             WHERE chat_members.room_id = $1 AND chat_members.status = 'active' \
             ORDER BY LOWER(users.username)",
            )
            .bind(room_id)
            .fetch_all(pool)
            .await
        })?;
        Ok(rows.into_iter().map(ParticipantRow::into_member).collect())
    }

    pub async fn remove_chat_participant(
        &self,
        room_id: Uuid,
        user_id: Uuid,
    ) -> Result<bool, sqlx::Error> {
        Ok(self
            .delete_chat_membership(room_id, user_id, false)
            .await?
            .is_some())
    }

    pub(crate) async fn is_chat_participant(
        &self,
        room_id: Uuid,
        user_id: Uuid,
    ) -> Result<bool, sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM chat_members WHERE room_id = $1 AND user_id = $2 AND status = 'active')")
            .bind(room_id)
            .bind(user_id)
            .fetch_one(pool)
            .await
        })
    }
}

#[derive(FromRow)]
struct ParticipantRow {
    user_id: Uuid,
    username: String,
    avatar_emoji: String,
}

impl ParticipantRow {
    fn into_member(self) -> ChatMember {
        ChatMember {
            user_id: self.user_id,
            username: self.username,
            avatar_emoji: self.avatar_emoji,
        }
    }
}
