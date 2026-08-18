//! Durable room participant roster used by group read receipts.

use sqlx::FromRow;
use uuid::Uuid;

use crate::models::RoomMember;
use crate::state::AppState;

impl AppState {
    pub async fn room_participants(&self, room_id: Uuid) -> Result<Vec<RoomMember>, sqlx::Error> {
        let rows: Vec<ParticipantRow> = sqlx::query_as(
            "SELECT users.id AS user_id, users.username, users.avatar_emoji \
             FROM room_memberships JOIN users ON users.id = room_memberships.user_id \
             WHERE room_memberships.room_id = ? AND room_memberships.status = 'active' \
             ORDER BY users.username COLLATE NOCASE",
        )
        .bind(room_id)
        .fetch_all(self.pool())
        .await?;
        Ok(rows.into_iter().map(ParticipantRow::into_member).collect())
    }

    pub async fn remove_room_participant(
        &self,
        room_id: Uuid,
        user_id: Uuid,
    ) -> Result<bool, sqlx::Error> {
        Ok(self
            .delete_room_membership(room_id, user_id, false)
            .await?
            .is_some())
    }

    pub(crate) async fn is_room_participant(
        &self,
        room_id: Uuid,
        user_id: Uuid,
    ) -> Result<bool, sqlx::Error> {
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM room_memberships WHERE room_id = ? AND user_id = ? AND status = 'active')")
            .bind(room_id)
            .bind(user_id)
            .fetch_one(self.pool())
            .await
    }
}

#[derive(FromRow)]
struct ParticipantRow {
    user_id: Uuid,
    username: String,
    avatar_emoji: String,
}

impl ParticipantRow {
    fn into_member(self) -> RoomMember {
        RoomMember {
            user_id: self.user_id,
            username: self.username,
            avatar_emoji: self.avatar_emoji,
        }
    }
}
