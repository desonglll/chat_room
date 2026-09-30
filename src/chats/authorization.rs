//! The chat authorization decision, in the order `docs/tg/architecture.md` §4.4 fixes.
//!
//! One function answers "may this user do this in this chat", and every read path and write
//! path calls it — `AGENTS.md` requires authorization at read time as well as write time, so
//! there is deliberately no cheaper variant for reads to reach for.

use uuid::Uuid;

use super::chat_type::ChatType;
use crate::state::{with_pool, AppState};

/// Why a chat action was allowed or refused. The variant is the audit trail: a caller that
/// only needs a boolean uses [`ChatAuthorization::is_allowed`], and a caller that has to
/// explain itself keeps the variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatAuthorization {
    /// Step 1 — a deployment-wide administrator, acting independently of any chat role.
    SystemAdministrator,
    /// Step 2 — the account recorded in `chats.creator_user_id`.
    Creator,
    /// Step 3 — a role in this chat carries the permission.
    RolePermission,
    /// Step 3 — no role in this chat carries the permission for this account.
    NoRolePermission,
    /// Step 5 — something granted the permission, but the chat's type cannot do it at all.
    ForbiddenByChatType(ChatType),
}

impl ChatAuthorization {
    pub const fn is_allowed(self) -> bool {
        matches!(
            self,
            ChatAuthorization::SystemAdministrator
                | ChatAuthorization::Creator
                | ChatAuthorization::RolePermission
        )
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            ChatAuthorization::SystemAdministrator => "system_administrator",
            ChatAuthorization::Creator => "creator",
            ChatAuthorization::RolePermission => "role_permission",
            ChatAuthorization::NoRolePermission => "no_role_permission",
            ChatAuthorization::ForbiddenByChatType(_) => "forbidden_by_chat_type",
        }
    }
}

/// Permissions a system administrator holds in every chat without a membership.
///
/// Deliberately administrative only. `CONTEXT.md` defines a System Administrator as a user
/// "entrusted with deployment-wide **operations** independently of any Room role", and
/// `AGENTS.md` keeps the chat as the knowledge-isolation boundary — so the step 1 shortcut
/// never grants a `message.*` permission. A system administrator cannot read, send, edit or
/// pin another chat's messages through this path.
const SYSTEM_ADMINISTRATOR_PERMISSIONS: &[&str] = &[
    "room.settings",
    "room.delete",
    "members.review",
    "members.invite",
    "members.remove",
    "members.roles",
];

impl AppState {
    /// Decide one chat action. The five steps run in this order and an earlier step never
    /// defers to a later one, except that step 5 can always veto:
    ///
    /// 1. system administrator
    /// 2. chat creator
    /// 3. role permission from `chat_role_permissions`
    /// 4. per-user restriction — **reserved for M2** (`chat_member_restrictions`); it belongs
    ///    between 3 and 5 and nowhere else, because it must be able to override a role grant
    ///    while still being subject to the intrinsic constraint
    /// 5. `chat_type` intrinsic constraint, which can only turn an allow into a deny
    pub async fn authorize_chat_action(
        &self,
        room_id: Uuid,
        user_id: Uuid,
        permission: &str,
    ) -> Result<ChatAuthorization, sqlx::Error> {
        let granted = self
            .chat_permission_grant(room_id, user_id, permission)
            .await?;
        if !granted.is_allowed() {
            return Ok(granted);
        }
        // Step 5. Applies to every grant above it, including a system administrator's: no
        // account can pin a forum topic in a one-to-one chat, because the chat cannot hold one.
        let chat_type = self.chat_type(room_id).await?;
        match chat_type {
            Some(chat_type) if !chat_type.permits(permission) => {
                Ok(ChatAuthorization::ForbiddenByChatType(chat_type))
            }
            _ => Ok(granted),
        }
    }

    /// Steps 1 to 3. Step 4 slots in at the end of this function when M2 adds the table.
    async fn chat_permission_grant(
        &self,
        room_id: Uuid,
        user_id: Uuid,
        permission: &str,
    ) -> Result<ChatAuthorization, sqlx::Error> {
        if SYSTEM_ADMINISTRATOR_PERMISSIONS.contains(&permission)
            && self.is_system_admin(user_id).await?
        {
            return Ok(ChatAuthorization::SystemAdministrator);
        }
        if self.is_chat_creator(room_id, user_id).await? {
            return Ok(ChatAuthorization::Creator);
        }
        if self
            .has_chat_role_permission(room_id, user_id, permission)
            .await?
        {
            return Ok(ChatAuthorization::RolePermission);
        }
        Ok(ChatAuthorization::NoRolePermission)
    }

    /// The boolean form every handler calls. Identical decision, discarded reason.
    pub async fn has_chat_permission(
        &self,
        room_id: Uuid,
        user_id: Uuid,
        permission: &str,
    ) -> Result<bool, sqlx::Error> {
        Ok(self
            .authorize_chat_action(room_id, user_id, permission)
            .await?
            .is_allowed())
    }

    /// Step 3 on its own — the pre-TG-005 behaviour, kept separate so the decision order is
    /// readable rather than buried in one query.
    pub async fn has_chat_role_permission(
        &self,
        room_id: Uuid,
        user_id: Uuid,
        permission: &str,
    ) -> Result<bool, sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM chat_members \
                 JOIN chat_role_permissions \
                   ON chat_role_permissions.role_id = chat_members.role_id \
                 WHERE chat_members.room_id = $1 AND chat_members.user_id = $2 \
                 AND chat_members.status = 'active' \
                 AND chat_role_permissions.permission_key = $3)",
            )
            .bind(room_id)
            .bind(user_id)
            .bind(permission)
            .fetch_one(pool)
            .await
        })
    }

    /// Step 2. A creator who is still an active member is already covered by step 3; this
    /// matters when the owner role was reassigned but `chats.creator_user_id` still points at
    /// the founder.
    pub async fn is_chat_creator(&self, room_id: Uuid, user_id: Uuid) -> Result<bool, sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM chats \
                 WHERE id = $1 AND creator_user_id = $2 AND deleted_at IS NULL)",
            )
            .bind(room_id)
            .bind(user_id)
            .fetch_one(pool)
            .await
        })
    }

    /// `None` when the chat does not exist or is soft-deleted, in which case there is no
    /// intrinsic constraint to apply and the earlier steps already decided.
    pub async fn chat_type(&self, room_id: Uuid) -> Result<Option<ChatType>, sqlx::Error> {
        let stored: Option<String> = with_pool!(self, |pool| {
            sqlx::query_scalar("SELECT chat_type FROM chats WHERE id = $1 AND deleted_at IS NULL")
                .bind(room_id)
                .fetch_optional(pool)
                .await
        })?;
        Ok(stored.and_then(|value| value.parse().ok()))
    }
}
