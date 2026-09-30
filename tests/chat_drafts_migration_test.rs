//! TG-008: the `chat_drafts` table (`20261001000004`) on both adapters, fresh and upgraded.
//!
//! Fresh path: `AppState::open`/`open_postgres` on an empty scratch database applies every
//! migration. Upgrade path: the pre-TG-004 schema (the version every real deployment is
//! upgrading from) with seeded rows, carried through the rename wave and this task's
//! migration in one go. Both paths then exercise the schema semantics that matter to the
//! drafts module: the composite primary key, the cascade from `chats` and `users`, and the
//! `SET NULL` from `messages`.

mod migration_support;

use chat_room::{config::AppConfig, state::AppState};
use chrono::Utc;
use migration_support::{
    chat_schema::{seed_postgres_pre_rename, seed_sqlite_pre_rename},
    create_postgres_scratch, drop_postgres_scratch, migration_count, migrations_path,
    migrations_through, postgres_admin_pool, postgres_pool, remove_sqlite_files, sqlite_pool,
    sqlite_scratch_path, PRE_TG_004_VERSION,
};
use sqlx::migrate::Migrator;
use uuid::Uuid;

const DRAFT_COLUMNS: [&str; 6] = [
    "room_id",
    "user_id",
    "text",
    "reply_to_message_id",
    "topic_id",
    "updated_at",
];

/// Seed a (user, chat, membershipless message) triple and a draft pointing at all three,
/// then prove the FK actions: deleting the message nulls the reply pointer, deleting the
/// chat removes the draft, and a second insert for the same (chat, account) conflicts on
/// the primary key rather than duplicating. Pure SQL so it runs identically on both pools.
macro_rules! assert_draft_semantics {
    ($pool:expr) => {{
        let now = Utc::now();
        let (user, chat, message) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
        sqlx::query(
            "INSERT INTO users (id, username, password_hash, created_at) VALUES ($1, $2, 'x', $3)",
        )
        .bind(user)
        .bind(format!("tg008-{}", user.simple()))
        .bind(now)
        .execute($pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO chats (id, title, password_hash, creator_user_id, join_policy, \
             created_at) VALUES ($1, $2, '', $3, 'open', $4)",
        )
        .bind(chat)
        .bind(format!("tg008-chat-{}", chat.simple()))
        .bind(user)
        .bind(now)
        .execute($pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO messages (id, room_id, sender_id, sender, content, created_at) \
             VALUES ($1, $2, $3, 'tg008', 'reply target', $4)",
        )
        .bind(message)
        .bind(chat)
        .bind(user)
        .bind(now)
        .execute($pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO chat_drafts \
             (room_id, user_id, text, reply_to_message_id, topic_id, updated_at) \
             VALUES ($1, $2, 'unsent', $3, NULL, $4)",
        )
        .bind(chat)
        .bind(user)
        .bind(message)
        .bind(now)
        .execute($pool)
        .await
        .unwrap();

        let duplicate = sqlx::query(
            "INSERT INTO chat_drafts (room_id, user_id, text, updated_at) \
             VALUES ($1, $2, 'second', $3)",
        )
        .bind(chat)
        .bind(user)
        .bind(now)
        .execute($pool)
        .await;
        assert!(
            duplicate.is_err(),
            "the (room_id, user_id) primary key must reject a second draft per chat/account"
        );

        sqlx::query("DELETE FROM messages WHERE id = $1")
            .bind(message)
            .execute($pool)
            .await
            .unwrap();
        let reply: Option<Uuid> = sqlx::query_scalar(
            "SELECT reply_to_message_id FROM chat_drafts WHERE room_id = $1 AND user_id = $2",
        )
        .bind(chat)
        .bind(user)
        .fetch_one($pool)
        .await
        .unwrap();
        assert_eq!(reply, None, "deleting the reply target must SET NULL");

        sqlx::query("DELETE FROM chats WHERE id = $1")
            .bind(chat)
            .execute($pool)
            .await
            .unwrap();
        let remaining: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM chat_drafts WHERE room_id = $1")
                .bind(chat)
                .fetch_one($pool)
                .await
                .unwrap();
        assert_eq!(remaining, 0, "deleting the chat must cascade to its drafts");
    }};
}

async fn assert_sqlite_draft_schema(pool: &sqlx::SqlitePool) {
    let columns: i64 = sqlx::query_scalar(&format!(
        "SELECT COUNT(*) FROM pragma_table_info('chat_drafts') WHERE name IN ({})",
        DRAFT_COLUMNS.map(|c| format!("'{c}'")).join(", ")
    ))
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(columns as usize, DRAFT_COLUMNS.len());
    let index: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = 'chat_drafts_user_idx'",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(index, 1);
}

async fn assert_postgres_draft_schema(pool: &sqlx::PgPool) {
    let columns: i64 = sqlx::query_scalar(&format!(
        "SELECT COUNT(*) FROM information_schema.columns WHERE table_schema = 'public' \
         AND table_name = 'chat_drafts' AND column_name IN ({})",
        DRAFT_COLUMNS.map(|c| format!("'{c}'")).join(", ")
    ))
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(columns as usize, DRAFT_COLUMNS.len());
    let index: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM pg_indexes WHERE schemaname = 'public' \
         AND indexname = 'chat_drafts_user_idx'",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(index, 1);
}

#[tokio::test]
async fn sqlite_fresh_schema_has_chat_drafts() {
    let database = sqlite_scratch_path("drafts-fresh");
    let state = AppState::open(&database).await.unwrap();
    let applied: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM _sqlx_migrations")
        .fetch_one(state.pool())
        .await
        .unwrap();
    assert_eq!(
        applied,
        migration_count(&migrations_path("migrations")).await
    );
    assert_sqlite_draft_schema(state.pool()).await;
    assert_draft_semantics!(state.pool());
    state.pool().close().await;
    remove_sqlite_files(&database);
}

#[tokio::test]
async fn sqlite_upgrade_from_the_pre_tg_004_schema_gains_chat_drafts() {
    let database = sqlite_scratch_path("drafts-upgrade");
    let pool = sqlite_pool(&database).await;
    let directory = migrations_path("migrations");
    migrations_through(&directory, PRE_TG_004_VERSION)
        .await
        .run(&pool)
        .await
        .unwrap();
    let _seeded = seed_sqlite_pre_rename(&pool).await;
    pool.close().await;

    let state = AppState::open(&database).await.unwrap();
    let applied: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM _sqlx_migrations")
        .fetch_one(state.pool())
        .await
        .unwrap();
    assert_eq!(applied, migration_count(&directory).await);
    assert_sqlite_draft_schema(state.pool()).await;
    assert_draft_semantics!(state.pool());
    state.pool().close().await;
    remove_sqlite_files(&database);
}

#[tokio::test]
async fn postgres_fresh_schema_has_chat_drafts() {
    let Some((admin_url, admin_pool)) =
        postgres_admin_pool("postgres_fresh_schema_has_chat_drafts").await
    else {
        return;
    };
    let scratch = create_postgres_scratch(&admin_url, &admin_pool, "drafts_fresh").await;
    let state = AppState::open_postgres(&scratch.url, &AppConfig::default())
        .await
        .unwrap();
    let postgres = state.postgres_pool().unwrap();
    let applied: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM _sqlx_migrations")
        .fetch_one(postgres)
        .await
        .unwrap();
    assert_eq!(
        applied,
        migration_count(&migrations_path("migrations-postgres")).await
    );
    assert_postgres_draft_schema(postgres).await;
    assert_draft_semantics!(postgres);
    postgres.close().await;
    drop(state);
    drop_postgres_scratch(&admin_pool, &scratch).await;
}

#[tokio::test]
async fn postgres_upgrade_from_the_pre_tg_004_schema_gains_chat_drafts() {
    let Some((admin_url, admin_pool)) =
        postgres_admin_pool("postgres_upgrade_from_the_pre_tg_004_schema_gains_chat_drafts").await
    else {
        return;
    };
    let scratch = create_postgres_scratch(&admin_url, &admin_pool, "drafts_upgrade").await;
    let pool = postgres_pool(&scratch.url).await;
    let directory = migrations_path("migrations-postgres");
    migrations_through(&directory, PRE_TG_004_VERSION)
        .await
        .run(&pool)
        .await
        .unwrap();
    let _seeded = seed_postgres_pre_rename(&pool).await;
    pool.close().await;

    let state = AppState::open_postgres(&scratch.url, &AppConfig::default())
        .await
        .unwrap();
    let postgres = state.postgres_pool().unwrap();
    let applied: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM _sqlx_migrations")
        .fetch_one(postgres)
        .await
        .unwrap();
    assert_eq!(applied, migration_count(&directory).await);
    assert_postgres_draft_schema(postgres).await;
    assert_draft_semantics!(postgres);
    postgres.close().await;
    drop(state);
    drop_postgres_scratch(&admin_pool, &scratch).await;
}

/// Both directories must hold the same version+name pair — the parity script checks names,
/// this pins the number this task reserved on the card.
#[tokio::test]
async fn the_draft_migration_exists_in_both_adapters_under_the_reserved_version() {
    for directory in ["migrations", "migrations-postgres"] {
        let migrator = Migrator::new(migrations_path(directory).as_path())
            .await
            .unwrap();
        assert!(
            migrator
                .iter()
                .any(|migration| migration.version == 20261001000004
                    && migration.description == "add chat drafts"),
            "{directory} is missing 20261001000004_add_chat_drafts"
        );
    }
}
