use chrono::Utc;
use std::collections::HashSet;
use uuid::Uuid;

use crate::models::Room;
use crate::social::canonical_pair;
use crate::state::{with_pool, AppState};

const MEMBER_PERMISSIONS: &[&str] = &["message.send", "message.edit_own", "message.recall_own"];

impl AppState {
    pub async fn start_direct_conversation(
        &self,
        user_id: Uuid,
        peer_id: Uuid,
    ) -> Result<Uuid, sqlx::Error> {
        let (low, high) = canonical_pair(user_id, peer_id);
        let now = Utc::now();
        let mut created_room = None;
        let room_id = with_pool!(self, |pool| {
            let mut transaction = pool.begin().await?;
            let friendship_locked = sqlx::query(
                "UPDATE friendships SET updated_at = updated_at \
                 WHERE user_low_id = $1 AND user_high_id = $2 AND status = 'accepted'",
            )
            .bind(low)
            .bind(high)
            .execute(&mut *transaction)
            .await?
            .rows_affected();
            if friendship_locked == 0 {
                return Err(sqlx::Error::RowNotFound);
            }
            let blocked: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM user_blocks WHERE \
                   (blocker_id = $1 AND blocked_id = $2) OR \
                   (blocker_id = $2 AND blocked_id = $1))",
            )
            .bind(low)
            .bind(high)
            .fetch_one(&mut *transaction)
            .await?;
            if blocked {
                return Err(sqlx::Error::RowNotFound);
            }
            let existing: Option<Uuid> = sqlx::query_scalar(
                "SELECT direct_conversations.room_id FROM direct_conversations \
                 JOIN rooms ON rooms.id = direct_conversations.room_id \
                 WHERE user_low_id = $1 AND user_high_id = $2 AND rooms.deleted_at IS NULL",
            )
            .bind(low)
            .bind(high)
            .fetch_optional(&mut *transaction)
            .await?;
            let room_id = match existing {
                Some(room_id) => room_id,
                None => {
                    let room_id = Uuid::new_v4();
                    let internal_name = format!("direct-{}", room_id.simple());
                    sqlx::query(
                        "INSERT INTO rooms (id, name, password_hash, creator_user_id, \
                         join_policy, avatar_emoji, description, created_at) \
                         VALUES ($1, $2, '', NULL, 'approval', '', '', $3)",
                    )
                    .bind(room_id)
                    .bind(&internal_name)
                    .bind(now)
                    .execute(&mut *transaction)
                    .await?;
                    let role_id = format!("{}:member", room_id.simple());
                    sqlx::query(
                        "INSERT INTO room_roles (id, room_id, name, is_system, created_at) \
                         VALUES ($1, $2, 'member', TRUE, $3)",
                    )
                    .bind(&role_id)
                    .bind(room_id)
                    .bind(now)
                    .execute(&mut *transaction)
                    .await?;
                    for permission in MEMBER_PERMISSIONS {
                        sqlx::query(
                            "INSERT INTO room_role_permissions (role_id, permission_key) \
                             VALUES ($1, $2)",
                        )
                        .bind(&role_id)
                        .bind(permission)
                        .execute(&mut *transaction)
                        .await?;
                    }
                    sqlx::query(
                        "INSERT INTO direct_conversations \
                         (room_id, user_low_id, user_high_id, created_at) \
                         VALUES ($1, $2, $3, $4)",
                    )
                    .bind(room_id)
                    .bind(low)
                    .bind(high)
                    .bind(now)
                    .execute(&mut *transaction)
                    .await?;
                    created_room = Some(Room {
                        id: room_id,
                        name: internal_name,
                        password_hash: String::new(),
                        has_password: false,
                        creator_user_id: None,
                        join_policy: "approval".into(),
                        avatar_emoji: String::new(),
                        description: String::new(),
                        membership_status: None,
                        membership_role: None,
                        unread_count: 0,
                        created_at: now,
                    });
                    room_id
                }
            };
            let role_id = format!("{}:member", room_id.simple());
            for member_id in [low, high] {
                sqlx::query(
                    "INSERT INTO room_memberships \
                     (room_id, user_id, role_id, status, requested_at, joined_at) \
                     VALUES ($1, $2, $3, 'active', $4, $5) \
                     ON CONFLICT(room_id, user_id) DO UPDATE SET status = 'active', \
                       role_id = excluded.role_id, \
                       joined_at = COALESCE(room_memberships.joined_at, excluded.joined_at)",
                )
                .bind(room_id)
                .bind(member_id)
                .bind(&role_id)
                .bind(now)
                .bind(now)
                .execute(&mut *transaction)
                .await?;
            }
            transaction.commit().await?;
            Ok::<_, sqlx::Error>(room_id)
        })?;
        if let Some(room) = created_room {
            self.cache_inserted_room(room).await;
        }
        Ok(room_id)
    }

    pub async fn is_direct_room(&self, room_id: Uuid) -> Result<bool, sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM direct_conversations WHERE room_id = $1)",
            )
            .bind(room_id)
            .fetch_one(pool)
            .await
        })
    }

    pub async fn direct_room_ids(&self) -> Result<HashSet<Uuid>, sqlx::Error> {
        let ids: Vec<Uuid> = with_pool!(self, |pool| {
            sqlx::query_scalar("SELECT room_id FROM direct_conversations")
                .fetch_all(pool)
                .await
        })?;
        Ok(ids.into_iter().collect())
    }
}
