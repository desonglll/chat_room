//! A group's default permissions: what every ordinary member may do.
//!
//! Stored as the `member` role's grants — the RBAC registry is extended, not shadowed by a
//! second mask (`docs/tg/architecture.md` §4.4). Only `permissions::MEMBER_TOGGLEABLE` keys are
//! ever added or removed here; the baseline (acting on one's own messages) always stays.

use uuid::Uuid;

use super::admin_models::ChatPermissionsView;
use super::permissions::{MEMBER_TOGGLEABLE, REGISTRY};
use super::provisioning::system_role_id;
use crate::state::{with_pool, AppState};

impl AppState {
    /// The toggleable keys the `member` role currently holds, in registry order.
    pub async fn chat_default_permissions(
        &self,
        room_id: Uuid,
    ) -> Result<Vec<String>, sqlx::Error> {
        let granted: Vec<String> = with_pool!(self, |pool| {
            sqlx::query_scalar(
                "SELECT grants.permission_key FROM chat_role_permissions AS grants \
                 JOIN chat_roles AS roles ON roles.id = grants.role_id \
                 WHERE roles.room_id = $1 AND roles.name = 'member'",
            )
            .bind(room_id)
            .fetch_all(pool)
            .await
        })?;
        Ok(MEMBER_TOGGLEABLE
            .iter()
            .filter(|key| granted.iter().any(|granted| granted == *key))
            .map(|key| key.to_string())
            .collect())
    }

    /// Switch the toggleable keys of the `member` role to exactly `enabled`.
    pub async fn replace_chat_default_permissions(
        &self,
        room_id: Uuid,
        enabled: &[&str],
    ) -> Result<(), sqlx::Error> {
        let role_id = self
            .member_role_id(room_id)
            .await?
            .unwrap_or_else(|| system_role_id(room_id, "member"));
        with_pool!(self, |pool| {
            let mut transaction = pool.begin().await?;
            for key in MEMBER_TOGGLEABLE {
                sqlx::query(
                    "DELETE FROM chat_role_permissions WHERE role_id = $1 AND permission_key = $2",
                )
                .bind(role_id.as_str())
                .bind(*key)
                .execute(&mut *transaction)
                .await?;
            }
            for key in enabled {
                sqlx::query(
                    "INSERT INTO chat_role_permissions (role_id, permission_key) VALUES ($1, $2)",
                )
                .bind(role_id.as_str())
                .bind(*key)
                .execute(&mut *transaction)
                .await?;
            }
            transaction.commit().await
        })
    }

    async fn member_role_id(&self, room_id: Uuid) -> Result<Option<String>, sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query_scalar("SELECT id FROM chat_roles WHERE room_id = $1 AND name = 'member'")
                .bind(room_id)
                .fetch_optional(pool)
                .await
        })
    }

    /// Every registered key `user_id` holds right now, through the full decision.
    pub async fn effective_chat_permissions(
        &self,
        room_id: Uuid,
        user_id: Uuid,
    ) -> Result<Vec<String>, sqlx::Error> {
        let mut held = Vec::new();
        for descriptor in REGISTRY {
            if self
                .has_chat_permission(room_id, user_id, descriptor.key)
                .await?
            {
                held.push(descriptor.key.to_string());
            }
        }
        Ok(held)
    }

    /// The `GET /api/chats/:id/permissions` answer for one viewer.
    pub async fn chat_permissions_view(
        &self,
        room_id: Uuid,
        viewer_id: Uuid,
    ) -> Result<Option<ChatPermissionsView>, sqlx::Error> {
        self.sync_chat_projection(room_id, false).await?;
        let Some(chat) = self.chat(room_id).await else {
            return Ok(None);
        };
        let my_role = self
            .membership_identity(room_id, viewer_id)
            .await?
            .map(|(_, role)| role)
            .unwrap_or_default();
        Ok(Some(ChatPermissionsView {
            chat_id: room_id,
            chat_type: chat.chat_type,
            member_count: chat.member_count,
            default_permissions: self.chat_default_permissions(room_id).await?,
            my_permissions: self.effective_chat_permissions(room_id, viewer_id).await?,
            my_role,
            registry: REGISTRY.to_vec(),
        }))
    }
}
