//! What TG-004 must be true of the schema, asserted the same way on both adapters.
//!
//! Split from `mod.rs` so neither file carries two responsibilities (and neither crosses the
//! 350-line warning in `scripts/check_file_sizes.py`).

#![allow(dead_code)]

use chrono::{DateTime, Utc};
use sqlx::{PgPool, SqlitePool};
use uuid::Uuid;

/// Table names that must not survive migration `20261001000001`.
pub const RENAMED_AWAY: [&str; 11] = [
    "rooms",
    "room_participants",
    "room_memberships",
    "room_roles",
    "room_role_permissions",
    "room_permissions",
    "room_reads",
    "room_pins",
    "room_bans",
    "room_tasks",
    "room_ai_policies",
];

/// The names they must have afterwards.
pub const RENAMED_TO: [&str; 10] = [
    "chats",
    "chat_members",
    "chat_roles",
    "chat_role_permissions",
    "chat_permissions",
    "chat_reads",
    "chat_pins",
    "chat_bans",
    "chat_tasks",
    "chat_ai_policies",
];

/// Columns migration `20261001000002` adds, plus the renamed `title`.
pub const CHAT_COLUMNS: [&str; 11] = [
    "chat_type",
    "title",
    "username",
    "access_hash",
    "is_forum",
    "linked_chat_id",
    "slow_mode_seconds",
    "auto_delete_seconds",
    "signatures_enabled",
    "history_visible_to_new_members",
    "member_count",
];

/// One group chat and one direct chat, with the owner membership each needs, written in the
/// *pre-rename* vocabulary so that the upgrade path has real rows to reclassify.
pub struct SeededChats {
    pub group_chat: Uuid,
    pub direct_chat: Uuid,
    pub owner: Uuid,
    pub peer: Uuid,
}

fn seed_ids() -> (SeededChats, DateTime<Utc>) {
    (
        SeededChats {
            group_chat: Uuid::new_v4(),
            direct_chat: Uuid::new_v4(),
            owner: Uuid::new_v4(),
            peer: Uuid::new_v4(),
        },
        Utc::now(),
    )
}

macro_rules! seed_pre_rename {
    ($pool:expr, $ids:expr, $now:expr) => {{
        let ids = $ids;
        let now = $now;
        for (id, username) in [(ids.owner, "tg004-owner"), (ids.peer, "tg004-peer")] {
            sqlx::query(
                "INSERT INTO users (id, username, password_hash, created_at) \
                 VALUES ($1, $2, 'x', $3)",
            )
            .bind(id)
            .bind(username)
            .bind(now)
            .execute($pool)
            .await
            .unwrap();
        }
        for (id, name) in [
            (ids.group_chat, "tg004-group"),
            (ids.direct_chat, "tg004-direct"),
        ] {
            sqlx::query(
                "INSERT INTO rooms (id, name, password_hash, creator_user_id, join_policy, \
                 created_at) VALUES ($1, $2, '', $3, 'open', $4)",
            )
            .bind(id)
            .bind(name)
            .bind(ids.owner)
            .bind(now)
            .execute($pool)
            .await
            .unwrap();
            sqlx::query(
                "INSERT INTO room_roles (id, room_id, name, is_system, created_at) \
                 VALUES ($1, $2, 'owner', TRUE, $3)",
            )
            .bind(format!("{}:owner", id.simple()))
            .bind(id)
            .bind(now)
            .execute($pool)
            .await
            .unwrap();
        }
        // Two active members in the group chat, one in the direct chat, so that the
        // member_count backfill has different answers to give.
        for (chat, user) in [
            (ids.group_chat, ids.owner),
            (ids.group_chat, ids.peer),
            (ids.direct_chat, ids.owner),
        ] {
            sqlx::query(
                "INSERT INTO room_memberships \
                 (room_id, user_id, role_id, status, requested_at, joined_at) \
                 VALUES ($1, $2, $3, 'active', $4, $4)",
            )
            .bind(chat)
            .bind(user)
            .bind(format!("{}:owner", chat.simple()))
            .bind(now)
            .execute($pool)
            .await
            .unwrap();
        }
        sqlx::query(
            "INSERT INTO direct_conversations (room_id, user_low_id, user_high_id, created_at) \
             VALUES ($1, $2, $3, $4)",
        )
        .bind(ids.direct_chat)
        .bind(ids.owner.min(ids.peer))
        .bind(ids.owner.max(ids.peer))
        .bind(now)
        .execute($pool)
        .await
        .unwrap();
        ids
    }};
}

pub async fn seed_sqlite_pre_rename(pool: &SqlitePool) -> SeededChats {
    let (ids, now) = seed_ids();
    seed_pre_rename!(pool, ids, now)
}

pub async fn seed_postgres_pre_rename(pool: &PgPool) -> SeededChats {
    let (ids, now) = seed_ids();
    seed_pre_rename!(pool, ids, now)
}

pub async fn assert_sqlite_chat_schema(pool: &SqlitePool) {
    for gone in RENAMED_AWAY {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = $1",
        )
        .bind(gone)
        .fetch_one(pool)
        .await
        .unwrap();
        assert_eq!(count, 0, "table {gone} still exists");
    }
    for present in RENAMED_TO {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = $1",
        )
        .bind(present)
        .fetch_one(pool)
        .await
        .unwrap();
        assert_eq!(count, 1, "table {present} is missing");
    }
    for column in CHAT_COLUMNS {
        let count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM pragma_table_info('chats') WHERE name = $1")
                .bind(column)
                .fetch_one(pool)
                .await
                .unwrap();
        assert_eq!(count, 1, "chats.{column} is missing");
    }
    let name_column: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM pragma_table_info('chats') WHERE name = 'name'")
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(name_column, 0, "chats.name was not renamed to title");
    for index in ["chats_title_active_idx", "chats_username_active_idx"] {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = $1",
        )
        .bind(index)
        .fetch_one(pool)
        .await
        .unwrap();
        assert_eq!(count, 1, "index {index} is missing");
    }
}

/// Proves the claim in `20261001000001`'s header comment instead of trusting it: after the
/// renames, no `REFERENCES` clause, trigger body or view definition anywhere in the schema
/// still names a table that no longer exists, and `foreign_key_check` finds no orphan.
pub async fn assert_sqlite_foreign_keys_resolve(pool: &SqlitePool) {
    let dangling: Vec<(String, String)> = sqlx::query_as(
        "SELECT source.name, fk.\"table\" FROM sqlite_master AS source \
         JOIN pragma_foreign_key_list(source.name) AS fk \
         WHERE source.type = 'table' \
           AND NOT EXISTS (SELECT 1 FROM sqlite_master AS target \
             WHERE target.type = 'table' AND target.name = fk.\"table\")",
    )
    .fetch_all(pool)
    .await
    .unwrap();
    assert!(
        dangling.is_empty(),
        "foreign keys point at tables that do not exist: {dangling:?}"
    );

    let stale: Vec<(String, String)> = sqlx::query_as(
        "SELECT type, name FROM sqlite_master WHERE sql IS NOT NULL AND ( \
           sql LIKE '%room_memberships%' OR sql LIKE '%room_roles%' \
           OR sql LIKE '%room_role_permissions%' OR sql LIKE '%room_permissions%' \
           OR sql LIKE '%room_reads%' OR sql LIKE '%room_pins%' OR sql LIKE '%room_bans%' \
           OR sql LIKE '%room_tasks%' OR sql LIKE '%room_ai_policies%' \
           OR sql LIKE '%REFERENCES rooms%' OR sql LIKE '%REFERENCES \"rooms\"%' \
           OR sql LIKE '%FROM rooms%' OR sql LIKE '%JOIN rooms%' OR sql LIKE '%INTO rooms%')",
    )
    .fetch_all(pool)
    .await
    .unwrap();
    assert!(
        stale.is_empty(),
        "schema objects still reference pre-rename table names: {stale:?}"
    );

    let violations: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM pragma_foreign_key_check")
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(violations, 0, "foreign_key_check reported orphaned rows");
}

pub async fn assert_postgres_chat_schema(pool: &PgPool) {
    for gone in RENAMED_AWAY {
        let found: Option<String> = sqlx::query_scalar("SELECT to_regclass('public.' || $1)::text")
            .bind(gone)
            .fetch_one(pool)
            .await
            .unwrap();
        assert_eq!(found, None, "table {gone} still exists");
    }
    for present in RENAMED_TO {
        let found: Option<String> = sqlx::query_scalar("SELECT to_regclass('public.' || $1)::text")
            .bind(present)
            .fetch_one(pool)
            .await
            .unwrap();
        assert_eq!(
            found.as_deref(),
            Some(present),
            "table {present} is missing"
        );
    }
    for column in CHAT_COLUMNS {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM information_schema.columns \
             WHERE table_schema = 'public' AND table_name = 'chats' AND column_name = $1",
        )
        .bind(column)
        .fetch_one(pool)
        .await
        .unwrap();
        assert_eq!(count, 1, "chats.{column} is missing");
    }
    let name_column: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM information_schema.columns \
         WHERE table_schema = 'public' AND table_name = 'chats' AND column_name = 'name'",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(name_column, 0, "chats.name was not renamed to title");
    for index in ["chats_title_active_idx", "chats_username_active_idx"] {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM pg_indexes WHERE schemaname = 'public' AND indexname = $1",
        )
        .bind(index)
        .fetch_one(pool)
        .await
        .unwrap();
        assert_eq!(count, 1, "index {index} is missing");
    }
}

/// PostgreSQL resolves foreign keys by OID, so a rename cannot dangle one — but it also does
/// not rewrite plpgsql bodies, which *can* be left pointing at a dropped name and only fail at
/// execution time. Both halves are checked.
pub async fn assert_postgres_foreign_keys_resolve(pool: &PgPool) {
    let to_chats: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM pg_constraint \
         WHERE contype = 'f' AND confrelid = 'public.chats'::regclass",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    assert!(
        to_chats >= 20,
        "expected the pre-rename foreign keys to chats to survive, found {to_chats}"
    );

    let stale: Vec<(String,)> = sqlx::query_as(
        "SELECT p.proname FROM pg_proc p JOIN pg_namespace n ON n.oid = p.pronamespace \
         WHERE n.nspname = 'public' AND p.prokind = 'f' AND p.prosrc ~ \
           '(room_memberships|room_roles|room_role_permissions|room_permissions|room_reads\
|room_pins|room_bans|room_tasks|room_ai_policies|\\mrooms\\M)'",
    )
    .fetch_all(pool)
    .await
    .unwrap();
    assert!(
        stale.is_empty(),
        "trigger functions still reference pre-rename table names: {stale:?}"
    );
}
