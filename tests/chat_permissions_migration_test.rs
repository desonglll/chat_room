//! TG-201: migrations `20261101000001` (permission keys, restrictions, admin rights) and
//! `20261101000002` (default permissions, member_count triggers) on both adapters, on a fresh
//! schema and upgraded from the pre-TG-004 baseline with seeded chats.

mod migration_support;

use chat_room::{config::AppConfig, state::AppState};
use chrono::Utc;
use migration_support::{
    chat_schema::{seed_postgres_pre_rename, seed_sqlite_pre_rename, SeededChats},
    create_postgres_scratch, drop_postgres_scratch, migrations_path, migrations_through,
    postgres_admin_pool, postgres_pool, remove_sqlite_files, sqlite_pool, sqlite_scratch_path,
    PRE_TG_004_VERSION,
};
use sqlx::migrate::Migrator;
use uuid::Uuid;

const M2_KEYS: [&str; 14] = [
    "message.post",
    "message.edit_any",
    "message.delete_any",
    "message.pin",
    "message.send_media",
    "message.send_sticker",
    "message.send_poll",
    "message.embed_link",
    "members.ban",
    "members.promote",
    "chat.info",
    "chat.topics",
    "chat.anonymous",
    "chat.call",
];

/// Schema-level facts that hold on both adapters. Pure SQL with `$n` placeholders.
macro_rules! assert_registry_and_tables {
    ($pool:expr) => {{
        for key in M2_KEYS {
            let registered: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM chat_permissions WHERE permission_key = $1)",
            )
            .bind(key)
            .fetch_one($pool)
            .await
            .unwrap();
            assert!(registered, "{key} registered");
        }
        let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM chat_permissions")
            .fetch_one($pool)
            .await
            .unwrap();
        assert_eq!(total, 23, "9 original + 14 M2 keys");
        for table in ["chat_member_restrictions", "chat_admin_rights"] {
            let rows: i64 = sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {table}"))
                .fetch_one($pool)
                .await
                .unwrap();
            assert_eq!(rows, 0, "{table} starts empty");
        }
    }};
}

/// The upgrade backfill and the triggers, against the chats `seed_*_pre_rename` created.
macro_rules! assert_upgraded_rows {
    ($pool:expr, $seeded:expr) => {{
        let seeded: &SeededChats = $seeded;
        let count = |chat: Uuid| {
            sqlx::query_scalar::<_, i64>(
                "SELECT CAST(member_count AS BIGINT) FROM chats WHERE id = $1",
            )
            .bind(chat)
        };
        assert_eq!(count(seeded.group_chat).fetch_one($pool).await.unwrap(), 2);
        assert_eq!(count(seeded.direct_chat).fetch_one($pool).await.unwrap(), 1);
        // The owner role of a pre-M2 chat now holds every registered key.
        let owner_keys: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM chat_role_permissions WHERE role_id = $1")
                .bind(format!("{}:owner", seeded.group_chat.simple()))
                .fetch_one($pool)
                .await
                .unwrap();
        assert_eq!(owner_keys, 23);

        // Triggers: a new active member counts, a pending one does not until approved, a
        // deleted account's cascade uncounts.
        let now = Utc::now();
        let joiner = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO users (id, username, password_hash, created_at) \
             VALUES ($1, 'tg201-joiner', 'x', $2)",
        )
        .bind(joiner)
        .bind(now)
        .execute($pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO chat_members (room_id, user_id, role_id, status, requested_at) \
             VALUES ($1, $2, $3, 'pending', $4)",
        )
        .bind(seeded.group_chat)
        .bind(joiner)
        .bind(format!("{}:owner", seeded.group_chat.simple()))
        .bind(now)
        .execute($pool)
        .await
        .unwrap();
        assert_eq!(count(seeded.group_chat).fetch_one($pool).await.unwrap(), 2);
        sqlx::query(
            "UPDATE chat_members SET status = 'active', joined_at = $1 \
             WHERE room_id = $2 AND user_id = $3",
        )
        .bind(now)
        .bind(seeded.group_chat)
        .bind(joiner)
        .execute($pool)
        .await
        .unwrap();
        assert_eq!(count(seeded.group_chat).fetch_one($pool).await.unwrap(), 3);
        sqlx::query(
            "INSERT INTO chat_admin_rights (room_id, user_id, permission_key) \
             VALUES ($1, $2, 'members.ban')",
        )
        .bind(seeded.group_chat)
        .bind(joiner)
        .execute($pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO chat_member_restrictions \
             (room_id, user_id, denied_permission_key, until, restricted_by, created_at) \
             VALUES ($1, $2, 'message.send', NULL, NULL, $3)",
        )
        .bind(seeded.group_chat)
        .bind(joiner)
        .bind(now)
        .execute($pool)
        .await
        .unwrap();
        sqlx::query("DELETE FROM users WHERE id = $1")
            .bind(joiner)
            .execute($pool)
            .await
            .unwrap();
        assert_eq!(count(seeded.group_chat).fetch_one($pool).await.unwrap(), 2);
        for table in ["chat_admin_rights", "chat_member_restrictions"] {
            let left: i64 =
                sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {table} WHERE user_id = $1"))
                    .bind(joiner)
                    .fetch_one($pool)
                    .await
                    .unwrap();
            assert_eq!(left, 0, "deleting the account cascades to {table}");
        }
    }};
}

async fn upgraded_migrator(directory: &str) -> Migrator {
    Migrator::new(migrations_path(directory)).await.unwrap()
}

#[tokio::test]
async fn sqlite_fresh_schema_has_the_permission_registry() {
    let database = sqlite_scratch_path("tg201-fresh");
    let state = AppState::open(&database).await.unwrap();
    assert_registry_and_tables!(state.pool());
    state.pool().close().await;
    remove_sqlite_files(&database);
}

#[tokio::test]
async fn sqlite_upgrade_recounts_members_and_installs_triggers() {
    let database = sqlite_scratch_path("tg201-upgrade");
    let pool = sqlite_pool(&database).await;
    migrations_through(&migrations_path("migrations"), PRE_TG_004_VERSION)
        .await
        .run(&pool)
        .await
        .unwrap();
    let seeded = seed_sqlite_pre_rename(&pool).await;
    upgraded_migrator("migrations")
        .await
        .run(&pool)
        .await
        .unwrap();
    assert_registry_and_tables!(&pool);
    assert_upgraded_rows!(&pool, &seeded);
    pool.close().await;
    remove_sqlite_files(&database);
}

#[tokio::test]
async fn postgres_fresh_schema_has_the_permission_registry() {
    let Some((admin_url, admin_pool)) =
        postgres_admin_pool("postgres_fresh_schema_has_the_permission_registry").await
    else {
        return;
    };
    let scratch = create_postgres_scratch(&admin_url, &admin_pool, "tg201_fresh").await;
    let state = AppState::open_postgres(&scratch.url, &AppConfig::default())
        .await
        .unwrap();
    assert_registry_and_tables!(state.postgres_pool().unwrap());
    state.postgres_pool().unwrap().close().await;
    drop(state);
    drop_postgres_scratch(&admin_pool, &scratch).await;
}

#[tokio::test]
async fn postgres_upgrade_recounts_members_and_installs_triggers() {
    let Some((admin_url, admin_pool)) =
        postgres_admin_pool("postgres_upgrade_recounts_members_and_installs_triggers").await
    else {
        return;
    };
    let scratch = create_postgres_scratch(&admin_url, &admin_pool, "tg201_upgrade").await;
    let pool = postgres_pool(&scratch.url).await;
    migrations_through(&migrations_path("migrations-postgres"), PRE_TG_004_VERSION)
        .await
        .run(&pool)
        .await
        .unwrap();
    let seeded = seed_postgres_pre_rename(&pool).await;
    upgraded_migrator("migrations-postgres")
        .await
        .run(&pool)
        .await
        .unwrap();
    assert_registry_and_tables!(&pool);
    assert_upgraded_rows!(&pool, &seeded);
    pool.close().await;
    drop_postgres_scratch(&admin_pool, &scratch).await;
}
