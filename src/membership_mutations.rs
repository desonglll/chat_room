//! Membership invitations, approvals, removals, and role changes.

use chrono::Utc;
use uuid::Uuid;

use crate::models::RoomMembership;
use crate::state::{with_pool, AppState};

impl AppState {
    pub async fn invite_room_member(
        &self,
        room_id: Uuid,
        invited_by: Uuid,
        username: &str,
    ) -> Result<Option<RoomMembership>, sqlx::Error> {
        let user_id: Option<Uuid> = with_pool!(self, |pool| {
            sqlx::query_scalar("SELECT id FROM users WHERE LOWER(username) = LOWER($1)")
                .bind(username)
                .fetch_optional(pool)
                .await
        })?;
        let Some(user_id) = user_id else {
            return Ok(None);
        };
        let now = Utc::now();
        with_pool!(self, |pool| { sqlx::query(
            "INSERT INTO room_memberships \
             (room_id, user_id, role_id, status, invited_by, requested_at) \
             SELECT $1, $2, room_roles.id, 'invited', $3, $4 FROM room_roles \
             WHERE room_roles.room_id = $5 AND room_roles.name = 'member' \
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
        .bind(invited_by)
        .bind(now)
        .bind(room_id)
        .execute(pool)
        .await
        .map(|_| ()) })?;
        self.room_membership(room_id, user_id).await
    }

    pub async fn activate_room_member(
        &self,
        room_id: Uuid,
        user_id: Uuid,
    ) -> Result<Option<RoomMembership>, sqlx::Error> {
        let changed = with_pool!(self, |pool| { sqlx::query(
            "UPDATE room_memberships SET status = 'active', joined_at = COALESCE(joined_at, $1) \
             WHERE room_id = $2 AND user_id = $3 AND status IN ('pending', 'invited')",
        )
        .bind(Utc::now())
        .bind(room_id)
        .bind(user_id)
        .execute(pool)
        .await
        .map(|result| result.rows_affected()) })?;
        if changed == 0 {
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
        let role_exists: bool = with_pool!(self, |pool| { sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM room_roles \
             WHERE room_id = $1 AND name = $2 AND name IN ('admin', 'member'))",
        )
        .bind(room_id)
        .bind(role)
        .fetch_one(pool)
        .await })?;
        if !role_exists {
            return Ok(None);
        }
        let changed = with_pool!(self, |pool| { sqlx::query(
            "UPDATE room_memberships SET role_id = (\
               SELECT id FROM room_roles WHERE room_id = $1 AND name = $2\
             ) WHERE room_id = $3 AND user_id = $4 AND status = 'active' \
             AND role_id <> (SELECT id FROM room_roles WHERE room_id = $5 AND name = 'owner')",
        )
        .bind(room_id)
        .bind(role)
        .bind(room_id)
        .bind(user_id)
        .bind(room_id)
        .execute(pool)
        .await
        .map(|result| result.rows_affected()) })?;
        if changed == 0 {
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
        with_pool!(self, |pool| {
            let mut transaction = pool.begin().await?;
            sqlx::query("DELETE FROM room_reads WHERE room_id = $1 AND user_id = $2")
                .bind(room_id)
                .bind(user_id)
                .execute(&mut *transaction)
                .await?;
            sqlx::query("DELETE FROM room_memberships WHERE room_id = $1 AND user_id = $2")
                .bind(room_id)
                .bind(user_id)
                .execute(&mut *transaction)
                .await?;
            transaction.commit().await?;
            Ok::<_, sqlx::Error>(())
        })?;
        Ok(Some(membership))
    }
}
