//! Creating a chat together with the three system roles and the owner's membership.
//!
//! The permission sets below are the registry rows `chat_role_permissions` points at.
//! `docs/tg/architecture.md` §4.4 hands the M2 expansion of this list to TG-201; the key
//! *values* (`room.settings`, `room.delete`) are frozen data that clients already consume.

use uuid::Uuid;

use crate::models::Chat;
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
    pub async fn create_chat_with_owner(
        &self,
        chat: Chat,
        owner_id: Uuid,
    ) -> Result<(), sqlx::Error> {
        with_pool!(self, |pool| {
            let mut transaction = pool.begin().await?;
            // member_count is seeded to 1 in the same transaction that inserts the owner's
            // membership: the projection is only trustworthy if it never lags the row that
            // created it.
            sqlx::query(
                "INSERT INTO chats \
             (id, chat_type, title, password_hash, creator_user_id, join_policy, avatar_emoji, \
              description, access_hash, member_count, created_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, 1, $10)",
            )
            .bind(chat.id)
            .bind(chat.chat_type.as_str())
            .bind(&chat.title)
            .bind(&chat.password_hash)
            .bind(owner_id)
            .bind(&chat.join_policy)
            .bind(&chat.avatar_emoji)
            .bind(&chat.description)
            .bind(&chat.access_hash)
            .bind(chat.created_at)
            .execute(&mut *transaction)
            .await?;

            let roles = [
                ("owner", OWNER_PERMISSIONS),
                ("admin", ADMIN_PERMISSIONS),
                ("member", MEMBER_PERMISSIONS),
            ];
            let mut owner_role_id = String::new();
            for (name, permissions) in roles {
                let role_id = format!("{}:{name}", chat.id.simple());
                if name == "owner" {
                    owner_role_id.clone_from(&role_id);
                }
                sqlx::query(
                    "INSERT INTO chat_roles (id, room_id, name, is_system, created_at) \
                 VALUES ($1, $2, $3, TRUE, $4)",
                )
                .bind(&role_id)
                .bind(chat.id)
                .bind(name)
                .bind(chat.created_at)
                .execute(&mut *transaction)
                .await?;
                for permission in permissions {
                    sqlx::query(
                    "INSERT INTO chat_role_permissions (role_id, permission_key) VALUES ($1, $2)",
                )
                .bind(&role_id)
                .bind(permission)
                .execute(&mut *transaction)
                .await?;
                }
            }
            sqlx::query(
                "INSERT INTO chat_members \
             (room_id, user_id, role_id, status, requested_at, joined_at) \
             VALUES ($1, $2, $3, 'active', $4, $5)",
            )
            .bind(chat.id)
            .bind(owner_id)
            .bind(owner_role_id)
            .bind(chat.created_at)
            .bind(chat.created_at)
            .execute(&mut *transaction)
            .await?;
            transaction.commit().await?;
            Ok::<_, sqlx::Error>(())
        })?;
        self.cache_inserted_chat(chat).await;
        Ok(())
    }
}
