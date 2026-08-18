//! Room-scoped roles, permissions, and membership persistence.

use std::collections::HashMap;

use chrono::Utc;
use uuid::Uuid;

use crate::models::{Room, RoomMembership};
use crate::state::AppState;

const MEMBER_PERMISSIONS: &[&str] = &["message.send", "message.edit_own", "message.recall_own"];
const ADMIN_PERMISSIONS: &[&str] = &[
    "message.send",
    "message.edit_own",
    "message.recall_own",
    "room.settings",
    "members.review",
    "members.invite",
    "members.remove",
];
const OWNER_PERMISSIONS: &[&str] = &[
    "message.send",
    "message.edit_own",
    "message.recall_own",
    "room.settings",
    "room.delete",
    "members.review",
    "members.invite",
    "members.remove",
    "members.roles",
];

impl AppState {
    pub async fn create_room_with_owner(
        &self,
        room: Room,
        owner_id: Uuid,
    ) -> Result<(), sqlx::Error> {
        let mut transaction = self.pool().begin().await?;
        sqlx::query(
            "INSERT INTO rooms \
             (id, name, password_hash, creator_user_id, join_policy, created_at) \
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(room.id)
        .bind(&room.name)
        .bind(&room.password_hash)
        .bind(owner_id)
        .bind(&room.join_policy)
        .bind(room.created_at)
        .execute(&mut *transaction)
        .await?;

        let roles = [
            ("owner", OWNER_PERMISSIONS),
            ("admin", ADMIN_PERMISSIONS),
            ("member", MEMBER_PERMISSIONS),
        ];
        let mut owner_role_id = String::new();
        for (name, permissions) in roles {
            let role_id = format!("{}:{name}", room.id.simple());
            if name == "owner" {
                owner_role_id.clone_from(&role_id);
            }
            sqlx::query(
                "INSERT INTO room_roles (id, room_id, name, is_system, created_at) \
                 VALUES (?, ?, ?, 1, ?)",
            )
            .bind(&role_id)
            .bind(room.id)
            .bind(name)
            .bind(room.created_at)
            .execute(&mut *transaction)
            .await?;
            for permission in permissions {
                sqlx::query(
                    "INSERT INTO room_role_permissions (role_id, permission_key) VALUES (?, ?)",
                )
                .bind(&role_id)
                .bind(permission)
                .execute(&mut *transaction)
                .await?;
            }
        }
        sqlx::query(
            "INSERT INTO room_memberships \
             (room_id, user_id, role_id, status, requested_at, joined_at) \
             VALUES (?, ?, ?, 'active', ?, ?)",
        )
        .bind(room.id)
        .bind(owner_id)
        .bind(owner_role_id)
        .bind(room.created_at)
        .bind(room.created_at)
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;
        self.cache_inserted_room(room).await;
        Ok(())
    }

    pub async fn membership_identity(
        &self,
        room_id: Uuid,
        user_id: Uuid,
    ) -> Result<Option<(String, String)>, sqlx::Error> {
        sqlx::query_as(
            "SELECT room_memberships.status, room_roles.name FROM room_memberships \
             JOIN room_roles ON room_roles.id = room_memberships.role_id \
             WHERE room_memberships.room_id = ? AND room_memberships.user_id = ?",
        )
        .bind(room_id)
        .bind(user_id)
        .fetch_optional(self.pool())
        .await
    }

    pub async fn decorate_rooms_for_user(
        &self,
        rooms: &mut [Room],
        user_id: Uuid,
    ) -> Result<(), sqlx::Error> {
        let rows: Vec<(Uuid, String, String)> = sqlx::query_as(
            "SELECT room_memberships.room_id, room_memberships.status, room_roles.name \
             FROM room_memberships JOIN room_roles ON room_roles.id = room_memberships.role_id \
             WHERE room_memberships.user_id = ?",
        )
        .bind(user_id)
        .fetch_all(self.pool())
        .await?;
        let identities: HashMap<_, _> = rows
            .into_iter()
            .map(|(room_id, status, role)| (room_id, (status, role)))
            .collect();
        for room in rooms.iter_mut() {
            if let Some((status, role)) = identities.get(&room.id) {
                room.membership_status = Some(status.clone());
                room.membership_role = Some(role.clone());
            }
        }
        let unread: HashMap<_, _> = self
            .room_unread_counts(user_id)
            .await?
            .into_iter()
            .collect();
        for room in rooms.iter_mut() {
            room.unread_count = unread.get(&room.id).copied().unwrap_or(0);
        }
        Ok(())
    }

    pub async fn room_unread_counts(&self, user_id: Uuid) -> Result<Vec<(Uuid, i64)>, sqlx::Error> {
        sqlx::query_as(
            "SELECT memberships.room_id, COUNT(messages.id) AS unread_count \
             FROM room_memberships AS memberships \
             LEFT JOIN room_reads ON room_reads.room_id = memberships.room_id \
               AND room_reads.user_id = memberships.user_id \
             LEFT JOIN messages AS read_message ON read_message.id = room_reads.message_id \
             LEFT JOIN messages ON messages.room_id = memberships.room_id \
               AND messages.recalled_at IS NULL \
               AND (messages.sender_id IS NULL OR messages.sender_id <> memberships.user_id) \
               AND (read_message.id IS NULL OR messages.created_at > read_message.created_at \
                 OR (messages.created_at = read_message.created_at AND messages.id > read_message.id)) \
             WHERE memberships.user_id = ? AND memberships.status = 'active' \
             GROUP BY memberships.room_id ORDER BY memberships.room_id",
        )
        .bind(user_id)
        .fetch_all(self.pool())
        .await
    }

    pub async fn account_membership_states(
        &self,
        user_id: Uuid,
    ) -> Result<Vec<(Uuid, String, String)>, sqlx::Error> {
        sqlx::query_as(
            "SELECT room_memberships.room_id, room_memberships.status, room_roles.name \
             FROM room_memberships JOIN room_roles ON room_roles.id = room_memberships.role_id \
             WHERE room_memberships.user_id = ? ORDER BY room_memberships.room_id",
        )
        .bind(user_id)
        .fetch_all(self.pool())
        .await
    }

    pub async fn has_room_permission(
        &self,
        room_id: Uuid,
        user_id: Uuid,
        permission: &str,
    ) -> Result<bool, sqlx::Error> {
        sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM room_memberships \
             JOIN room_role_permissions ON room_role_permissions.role_id = room_memberships.role_id \
             WHERE room_memberships.room_id = ? AND room_memberships.user_id = ? \
             AND room_memberships.status = 'active' \
             AND room_role_permissions.permission_key = ?)",
        )
        .bind(room_id)
        .bind(user_id)
        .bind(permission)
        .fetch_one(self.pool())
        .await
    }

    pub async fn request_room_membership(
        &self,
        room_id: Uuid,
        user_id: Uuid,
        auto_activate: bool,
    ) -> Result<RoomMembership, sqlx::Error> {
        let now = Utc::now();
        sqlx::query(
            "INSERT INTO room_memberships \
             (room_id, user_id, role_id, status, requested_at, joined_at) \
             SELECT ?, ?, room_roles.id, ?, ?, ? FROM room_roles \
             WHERE room_roles.room_id = ? AND room_roles.name = 'member' \
             ON CONFLICT(room_id, user_id) DO UPDATE SET \
               status = CASE \
                 WHEN room_memberships.status IN ('active', 'invited') OR ? THEN 'active' \
                 ELSE 'pending' END, \
               joined_at = CASE \
                 WHEN room_memberships.status IN ('active', 'invited') OR ? \
                 THEN COALESCE(room_memberships.joined_at, excluded.requested_at) \
                 ELSE room_memberships.joined_at END, \
               requested_at = excluded.requested_at",
        )
        .bind(room_id)
        .bind(user_id)
        .bind(if auto_activate { "active" } else { "pending" })
        .bind(now)
        .bind(auto_activate.then_some(now))
        .bind(room_id)
        .bind(auto_activate)
        .bind(auto_activate)
        .execute(self.pool())
        .await?;
        self.room_membership(room_id, user_id)
            .await?
            .ok_or(sqlx::Error::RowNotFound)
    }

    pub async fn room_membership(
        &self,
        room_id: Uuid,
        user_id: Uuid,
    ) -> Result<Option<RoomMembership>, sqlx::Error> {
        sqlx::query_as(
            "SELECT users.id AS user_id, users.username, users.avatar_emoji, \
             room_roles.name AS role, room_memberships.status, \
             room_memberships.requested_at, room_memberships.joined_at \
             FROM room_memberships JOIN users ON users.id = room_memberships.user_id \
             JOIN room_roles ON room_roles.id = room_memberships.role_id \
             WHERE room_memberships.room_id = ? AND room_memberships.user_id = ?",
        )
        .bind(room_id)
        .bind(user_id)
        .fetch_optional(self.pool())
        .await
    }

    pub async fn room_memberships(
        &self,
        room_id: Uuid,
    ) -> Result<Vec<RoomMembership>, sqlx::Error> {
        sqlx::query_as(
            "SELECT users.id AS user_id, users.username, users.avatar_emoji, \
             room_roles.name AS role, room_memberships.status, \
             room_memberships.requested_at, room_memberships.joined_at \
             FROM room_memberships JOIN users ON users.id = room_memberships.user_id \
             JOIN room_roles ON room_roles.id = room_memberships.role_id \
             WHERE room_memberships.room_id = ? \
             ORDER BY CASE room_memberships.status WHEN 'pending' THEN 0 WHEN 'invited' THEN 1 ELSE 2 END, \
             users.username COLLATE NOCASE",
        )
        .bind(room_id)
        .fetch_all(self.pool())
        .await
    }
}
