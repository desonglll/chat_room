//! Creating a chat together with its system roles and first memberships.
//!
//! The statement functions below are the only SQL that inserts a `chats` row, a system role,
//! its permission grants or a creation-time membership. Both a group created through
//! `POST /api/chats` and a private chat opened by `chats::private_chats` go through them
//! (TG-208), so the two kinds of chat cannot drift apart in how they are provisioned.
//!
//! The permission sets below are the registry rows `chat_role_permissions` points at.
//! `docs/tg/architecture.md` §4.4 hands the M2 expansion of this list to TG-201; the key
//! *values* (`room.settings`, `room.delete`) are frozen data that clients already consume.

use chrono::{DateTime, Utc};
use sqlx::{Database, Encode, Executor, IntoArguments, Type};
use uuid::Uuid;

use crate::models::Chat;
use crate::state::{with_pool, AppState};

pub(crate) const MEMBER_PERMISSIONS: &[&str] =
    &["message.send", "message.edit_own", "message.recall_own"];
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
            insert_chat_row(&mut *transaction, &chat, Some(owner_id), 1).await?;
            let roles = [
                ("owner", OWNER_PERMISSIONS),
                ("admin", ADMIN_PERMISSIONS),
                ("member", MEMBER_PERMISSIONS),
            ];
            for (name, permissions) in roles {
                let role_id = system_role_id(chat.id, name);
                insert_system_role(&mut *transaction, &role_id, chat.id, name, chat.created_at)
                    .await?;
                for permission in permissions {
                    grant_role_permission(&mut *transaction, &role_id, permission).await?;
                }
            }
            let owner_role_id = system_role_id(chat.id, "owner");
            upsert_active_member(
                &mut *transaction,
                chat.id,
                owner_id,
                &owner_role_id,
                chat.created_at,
            )
            .await?;
            transaction.commit().await?;
            Ok::<_, sqlx::Error>(())
        })?;
        self.cache_inserted_chat(chat).await;
        Ok(())
    }
}

/// `<chat id simple>:<role name>` — the id every system role of a chat has.
pub(crate) fn system_role_id(chat_id: Uuid, name: &str) -> String {
    format!("{}:{name}", chat_id.simple())
}

/// Insert the `chats` row. `member_count` must equal the memberships the same transaction
/// inserts, so the projection never lags the rows that created it.
pub(crate) async fn insert_chat_row<'c, E, DB>(
    executor: E,
    chat: &Chat,
    creator_user_id: Option<Uuid>,
    member_count: i64,
) -> Result<(), sqlx::Error>
where
    E: Executor<'c, Database = DB>,
    DB: Database,
    for<'q> DB::Arguments<'q>: IntoArguments<'q, DB>,
    for<'q> Uuid: Encode<'q, DB> + Type<DB>,
    for<'q> Option<Uuid>: Encode<'q, DB> + Type<DB>,
    for<'q> &'q str: Encode<'q, DB> + Type<DB>,
    for<'q> i64: Encode<'q, DB> + Type<DB>,
    for<'q> DateTime<Utc>: Encode<'q, DB> + Type<DB>,
{
    sqlx::query(
        "INSERT INTO chats \
         (id, chat_type, title, password_hash, creator_user_id, join_policy, avatar_emoji, \
          description, access_hash, member_count, created_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)",
    )
    .bind(chat.id)
    .bind(chat.chat_type.as_str())
    .bind(chat.title.as_str())
    .bind(chat.password_hash.as_str())
    .bind(creator_user_id)
    .bind(chat.join_policy.as_str())
    .bind(chat.avatar_emoji.as_str())
    .bind(chat.description.as_str())
    .bind(chat.access_hash.as_str())
    .bind(member_count)
    .bind(chat.created_at)
    .execute(executor)
    .await
    .map(|_| ())
}

pub(crate) async fn insert_system_role<'c, E, DB>(
    executor: E,
    role_id: &str,
    chat_id: Uuid,
    name: &str,
    created_at: DateTime<Utc>,
) -> Result<(), sqlx::Error>
where
    E: Executor<'c, Database = DB>,
    DB: Database,
    for<'q> DB::Arguments<'q>: IntoArguments<'q, DB>,
    for<'q> Uuid: Encode<'q, DB> + Type<DB>,
    for<'q> &'q str: Encode<'q, DB> + Type<DB>,
    for<'q> DateTime<Utc>: Encode<'q, DB> + Type<DB>,
{
    sqlx::query(
        "INSERT INTO chat_roles (id, room_id, name, is_system, created_at) \
         VALUES ($1, $2, $3, TRUE, $4)",
    )
    .bind(role_id)
    .bind(chat_id)
    .bind(name)
    .bind(created_at)
    .execute(executor)
    .await
    .map(|_| ())
}

pub(crate) async fn grant_role_permission<'c, E, DB>(
    executor: E,
    role_id: &str,
    permission_key: &str,
) -> Result<(), sqlx::Error>
where
    E: Executor<'c, Database = DB>,
    DB: Database,
    for<'q> DB::Arguments<'q>: IntoArguments<'q, DB>,
    for<'q> &'q str: Encode<'q, DB> + Type<DB>,
{
    sqlx::query("INSERT INTO chat_role_permissions (role_id, permission_key) VALUES ($1, $2)")
        .bind(role_id)
        .bind(permission_key)
        .execute(executor)
        .await
        .map(|_| ())
}

/// Make `user_id` an active member holding `role_id`, keeping the original `joined_at` if the
/// membership row already existed.
pub(crate) async fn upsert_active_member<'c, E, DB>(
    executor: E,
    chat_id: Uuid,
    user_id: Uuid,
    role_id: &str,
    now: DateTime<Utc>,
) -> Result<(), sqlx::Error>
where
    E: Executor<'c, Database = DB>,
    DB: Database,
    for<'q> DB::Arguments<'q>: IntoArguments<'q, DB>,
    for<'q> Uuid: Encode<'q, DB> + Type<DB>,
    for<'q> &'q str: Encode<'q, DB> + Type<DB>,
    for<'q> DateTime<Utc>: Encode<'q, DB> + Type<DB>,
{
    sqlx::query(
        "INSERT INTO chat_members \
         (room_id, user_id, role_id, status, requested_at, joined_at) \
         VALUES ($1, $2, $3, 'active', $4, $5) \
         ON CONFLICT(room_id, user_id) DO UPDATE SET status = 'active', \
           role_id = excluded.role_id, \
           joined_at = COALESCE(chat_members.joined_at, excluded.joined_at)",
    )
    .bind(chat_id)
    .bind(user_id)
    .bind(role_id)
    .bind(now)
    .bind(now)
    .execute(executor)
    .await
    .map(|_| ())
}
