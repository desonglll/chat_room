//! Membership invitations, approvals, removals, and role changes.

use chrono::Utc;
use uuid::Uuid;

use crate::models::RoomMembership;
use crate::state::AppState;

impl AppState {
    pub async fn invite_room_member(
        &self,
        room_id: Uuid,
        invited_by: Uuid,
        username: &str,
    ) -> Result<Option<RoomMembership>, sqlx::Error> {
        let user_id: Option<Uuid> =
            sqlx::query_scalar("SELECT id FROM users WHERE username = ? COLLATE NOCASE")
                .bind(username)
                .fetch_optional(self.pool())
                .await?;
        let Some(user_id) = user_id else {
            return Ok(None);
        };
        let role_id: String =
            sqlx::query_scalar("SELECT id FROM room_roles WHERE room_id = ? AND name = 'member'")
                .bind(room_id)
                .fetch_one(self.pool())
                .await?;
        let now = Utc::now();
        sqlx::query(
            "INSERT INTO room_memberships \
             (room_id, user_id, role_id, status, invited_by, requested_at) \
             VALUES (?, ?, ?, 'invited', ?, ?) \
             ON CONFLICT(room_id, user_id) DO UPDATE SET \
               status = CASE WHEN room_memberships.status = 'active' \
                 THEN 'active' ELSE 'invited' END, \
               invited_by = CASE WHEN room_memberships.status = 'active' \
                 THEN room_memberships.invited_by ELSE excluded.invited_by END, \
               requested_at = CASE WHEN room_memberships.status = 'active' \
                 THEN room_memberships.requested_at ELSE excluded.requested_at END",
        )
        .bind(room_id)
        .bind(user_id)
        .bind(role_id)
        .bind(invited_by)
        .bind(now)
        .execute(self.pool())
        .await?;
        self.room_membership(room_id, user_id).await
    }

    pub async fn activate_room_member(
        &self,
        room_id: Uuid,
        user_id: Uuid,
    ) -> Result<Option<RoomMembership>, sqlx::Error> {
        let result = sqlx::query(
            "UPDATE room_memberships SET status = 'active', joined_at = COALESCE(joined_at, ?) \
             WHERE room_id = ? AND user_id = ? AND status IN ('pending', 'invited')",
        )
        .bind(Utc::now())
        .bind(room_id)
        .bind(user_id)
        .execute(self.pool())
        .await?;
        if result.rows_affected() == 0 {
            return Ok(None);
        }
        self.room_membership(room_id, user_id).await
    }

    pub async fn set_room_member_role(
        &self,
        room_id: Uuid,
        user_id: Uuid,
        role: &str,
    ) -> Result<Option<RoomMembership>, sqlx::Error> {
        let role_id: Option<String> = sqlx::query_scalar(
            "SELECT id FROM room_roles WHERE room_id = ? AND name = ? AND name IN ('admin', 'member')",
        )
        .bind(room_id)
        .bind(role)
        .fetch_optional(self.pool())
        .await?;
        let Some(role_id) = role_id else {
            return Ok(None);
        };
        let result = sqlx::query(
            "UPDATE room_memberships SET role_id = ? \
             WHERE room_id = ? AND user_id = ? AND status = 'active' \
             AND role_id <> (SELECT id FROM room_roles WHERE room_id = ? AND name = 'owner')",
        )
        .bind(role_id)
        .bind(room_id)
        .bind(user_id)
        .bind(room_id)
        .execute(self.pool())
        .await?;
        if result.rows_affected() == 0 {
            return Ok(None);
        }
        self.room_membership(room_id, user_id).await
    }

    pub async fn delete_room_membership(
        &self,
        room_id: Uuid,
        user_id: Uuid,
        allow_owner: bool,
    ) -> Result<Option<RoomMembership>, sqlx::Error> {
        let membership = self.room_membership(room_id, user_id).await?;
        let Some(membership) = membership else {
            return Ok(None);
        };
        if membership.role == "owner" && !allow_owner {
            return Ok(None);
        }
        let mut transaction = self.pool().begin().await?;
        sqlx::query("DELETE FROM room_reads WHERE room_id = ? AND user_id = ?")
            .bind(room_id)
            .bind(user_id)
            .execute(&mut *transaction)
            .await?;
        sqlx::query("DELETE FROM room_memberships WHERE room_id = ? AND user_id = ?")
            .bind(room_id)
            .bind(user_id)
            .execute(&mut *transaction)
            .await?;
        transaction.commit().await?;
        Ok(Some(membership))
    }
}
