//! Room-scoped roles, permissions, and membership persistence.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::models::{Room, RoomMembership, User};
use crate::state::{with_pool, AppState};

const MEMBER_PERMISSIONS: &[&str] = &["message.send", "message.edit_own", "message.recall_own"];
const ADMIN_PERMISSIONS: &[&str] = &[
    "message.send",
    "message.edit_own",
    "message.recall_own",
    "message.pin",
    "room.settings",
    "members.review",
    "members.invite",
    "members.remove",
];
const OWNER_PERMISSIONS: &[&str] = &[
    "message.send",
    "message.edit_own",
    "message.recall_own",
    "message.pin",
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
        with_pool!(self, |pool| {
            let mut transaction = pool.begin().await?;
            sqlx::query(
            "INSERT INTO rooms \
             (id, name, password_hash, creator_user_id, join_policy, avatar_emoji, description, created_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
        )
        .bind(room.id)
        .bind(&room.name)
        .bind(&room.password_hash)
        .bind(owner_id)
        .bind(&room.join_policy)
        .bind(&room.avatar_emoji)
        .bind(&room.description)
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
                 VALUES ($1, $2, $3, TRUE, $4)",
                )
                .bind(&role_id)
                .bind(room.id)
                .bind(name)
                .bind(room.created_at)
                .execute(&mut *transaction)
                .await?;
                for permission in permissions {
                    sqlx::query(
                    "INSERT INTO room_role_permissions (role_id, permission_key) VALUES ($1, $2)",
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
             VALUES ($1, $2, $3, 'active', $4, $5)",
            )
            .bind(room.id)
            .bind(owner_id)
            .bind(owner_role_id)
            .bind(room.created_at)
            .bind(room.created_at)
            .execute(&mut *transaction)
            .await?;
            transaction.commit().await?;
            Ok::<_, sqlx::Error>(())
        })?;
        self.cache_inserted_room(room).await;
        Ok(())
    }

    pub async fn membership_identity(
        &self,
        room_id: Uuid,
        user_id: Uuid,
    ) -> Result<Option<(String, String)>, sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query_as(
                "SELECT room_memberships.status, room_roles.name FROM room_memberships \
             JOIN room_roles ON room_roles.id = room_memberships.role_id \
             WHERE room_memberships.room_id = $1 AND room_memberships.user_id = $2",
            )
            .bind(room_id)
            .bind(user_id)
            .fetch_optional(pool)
            .await
        })
    }

    pub async fn decorate_rooms_for_user(
        &self,
        rooms: &mut [Room],
        user_id: Uuid,
    ) -> Result<(), sqlx::Error> {
        let rows: Vec<(Uuid, String, String)> = with_pool!(self, |pool| {
            sqlx::query_as(
                "SELECT room_memberships.room_id, room_memberships.status, room_roles.name \
             FROM room_memberships JOIN room_roles ON room_roles.id = room_memberships.role_id \
             WHERE room_memberships.user_id = $1",
            )
            .bind(user_id)
            .fetch_all(pool)
            .await
        })?;
        let identities: HashMap<_, _> = rows
            .into_iter()
            .map(|(room_id, status, role)| (room_id, (status, role)))
            .collect();
        for room in rooms.iter_mut() {
            // Always overwrite, never only conditionally set: `rooms` may come
            // from the shared in-memory cache, which stores a room's freshly
            // created state including the *creator's own* membership fields
            // (see handlers::create_room) — a viewer with no real membership
            // must not inherit those stale values, or every room would look
            // joined (and private rooms would leak into everyone's listing).
            let identity = identities.get(&room.id);
            room.membership_status = identity.map(|(status, _)| status.clone());
            room.membership_role = identity.map(|(_, role)| role.clone());
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
        with_pool!(self, |pool| {
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
                   ELSE (SELECT COUNT(*) FROM room_memberships AS requests \
                     WHERE requests.room_id = memberships.room_id \
                       AND requests.status = 'pending') END AS BIGINT), \
                 CASE WHEN review.role_id IS NULL THEN NULL ELSE \
                   (SELECT MAX(requests.requested_at) FROM room_memberships AS requests \
                     WHERE requests.room_id = memberships.room_id \
                       AND requests.status = 'pending') END \
                 FROM room_memberships AS memberships \
                 JOIN room_roles AS roles ON roles.id = memberships.role_id \
                 LEFT JOIN room_role_permissions AS review ON review.role_id = roles.id \
                   AND review.permission_key = 'members.review' \
                 WHERE memberships.user_id = $1 ORDER BY memberships.room_id",
            )
            .bind(user_id)
            .fetch_all(pool)
            .await
        })
    }

    pub async fn has_room_permission(
        &self,
        room_id: Uuid,
        user_id: Uuid,
        permission: &str,
    ) -> Result<bool, sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM room_memberships \
             JOIN room_role_permissions ON room_role_permissions.role_id = room_memberships.role_id \
             WHERE room_memberships.room_id = $1 AND room_memberships.user_id = $2 \
             AND room_memberships.status = 'active' \
             AND room_role_permissions.permission_key = $3)",
        )
        .bind(room_id)
        .bind(user_id)
        .bind(permission)
        .fetch_one(pool)
        .await
        })
    }

    pub async fn request_room_membership(
        &self,
        room_id: Uuid,
        user_id: Uuid,
        auto_activate: bool,
    ) -> Result<RoomMembership, sqlx::Error> {
        if self.room_banned(room_id, user_id).await? {
            return Err(sqlx::Error::RowNotFound);
        }
        let now = Utc::now();
        let became_owner = with_pool!(self, |pool| {
            let mut transaction = pool.begin().await?;
            sqlx::query("UPDATE rooms SET creator_user_id = creator_user_id WHERE id = $1")
                .bind(room_id)
                .execute(&mut *transaction)
                .await?;
            let has_active_member: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM room_memberships \
                 WHERE room_id = $1 AND status = 'active')",
            )
            .bind(room_id)
            .fetch_one(&mut *transaction)
            .await?;
            let became_owner = !has_active_member;
            sqlx::query(
                "INSERT INTO room_memberships \
             (room_id, user_id, role_id, status, requested_at, joined_at) \
             SELECT $1, $2, room_roles.id, $3, $4, $5 FROM room_roles \
             WHERE room_roles.room_id = $6 AND room_roles.name = $9 \
             ON CONFLICT(room_id, user_id) DO UPDATE SET \
               role_id = CASE WHEN $10 THEN excluded.role_id ELSE room_memberships.role_id END, \
               status = CASE \
                 WHEN room_memberships.status IN ('active', 'invited') OR $7 THEN 'active' \
                 ELSE 'pending' END, \
               joined_at = CASE \
                 WHEN room_memberships.status IN ('active', 'invited') OR $8 \
                 THEN COALESCE(room_memberships.joined_at, excluded.requested_at) \
                 ELSE room_memberships.joined_at END, \
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
                sqlx::query("UPDATE rooms SET creator_user_id = $1 WHERE id = $2")
                    .bind(user_id)
                    .bind(room_id)
                    .execute(&mut *transaction)
                    .await?;
            }
            transaction.commit().await?;
            Ok::<_, sqlx::Error>(became_owner)
        })?;
        if became_owner {
            if let Some(mut room) = self.room(room_id).await {
                room.creator_user_id = Some(user_id);
                self.cache_updated_room(room).await;
            }
        }
        self.room_membership(room_id, user_id)
            .await?
            .ok_or(sqlx::Error::RowNotFound)
    }

    pub async fn room_membership(
        &self,
        room_id: Uuid,
        user_id: Uuid,
    ) -> Result<Option<RoomMembership>, sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query_as(
                "SELECT users.id AS user_id, users.username, users.avatar_emoji, \
             room_memberships.nickname, \
             room_roles.name AS role, room_memberships.status, \
             room_memberships.requested_at, room_memberships.joined_at \
             FROM room_memberships JOIN users ON users.id = room_memberships.user_id \
             JOIN room_roles ON room_roles.id = room_memberships.role_id \
             WHERE room_memberships.room_id = $1 AND room_memberships.user_id = $2",
            )
            .bind(room_id)
            .bind(user_id)
            .fetch_optional(pool)
            .await
        })
    }

    /// Set the caller's own room nickname without a management permission.
    pub async fn set_own_nickname(
        &self,
        room_id: Uuid,
        user_id: Uuid,
        nickname: &str,
    ) -> Result<Option<RoomMembership>, sqlx::Error> {
        let changed = with_pool!(self, |pool| {
            sqlx::query(
                "UPDATE room_memberships SET nickname = $1 \
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
        self.room_membership(room_id, user_id).await
    }

    /// Resolve and freeze nickname, display name, or username at message-send time.
    pub async fn resolve_display_name(&self, room_id: Uuid, user: &User) -> String {
        let nickname: Option<String> = with_pool!(self, |pool| {
            sqlx::query_scalar(
                "SELECT nickname FROM room_memberships WHERE room_id = $1 AND user_id = $2",
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

    pub async fn room_memberships(
        &self,
        room_id: Uuid,
    ) -> Result<Vec<RoomMembership>, sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query_as(
            "SELECT users.id AS user_id, users.username, users.avatar_emoji, \
             room_memberships.nickname, \
             room_roles.name AS role, room_memberships.status, \
             room_memberships.requested_at, room_memberships.joined_at \
             FROM room_memberships JOIN users ON users.id = room_memberships.user_id \
             JOIN room_roles ON room_roles.id = room_memberships.role_id \
             WHERE room_memberships.room_id = $1 \
             ORDER BY CASE room_memberships.status WHEN 'pending' THEN 0 WHEN 'invited' THEN 1 ELSE 2 END, \
             LOWER(users.username)",
        )
        .bind(room_id)
        .fetch_all(pool)
        .await
        })
    }
}
