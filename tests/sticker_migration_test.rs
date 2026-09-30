//! TG-302: the sticker tables (`20261201000001`) and `messages.media_kind` / `sticker_id`
//! (`20261201000002`) on both adapters, on a fresh database and on an upgrade from the schema
//! head just before them with an existing message.

mod migration_support;

use chat_room::{config::AppConfig, state::AppState};
use chrono::Utc;
use migration_support::{
    create_postgres_scratch, drop_postgres_scratch, migration_count, migrations_path,
    migrations_through, postgres_admin_pool, postgres_pool, remove_sqlite_files, sqlite_pool,
    sqlite_scratch_path,
};
use sqlx::migrate::Migrator;
use uuid::Uuid;

/// Everything before this task's reserved versions.
const BEFORE_TG_302: i64 = 20261201000000;

const TABLES: [&str; 8] = [
    "sticker_sets",
    "stickers",
    "sticker_emojis",
    "user_sticker_state",
    "user_sticker_sets",
    "user_recent_stickers",
    "user_favorite_stickers",
    "custom_emoji",
];

/// Seed a user, chat and plain message with the pre-TG-302 columns only.
macro_rules! seed_message {
    ($pool:expr) => {{
        let now = Utc::now();
        let (user, chat, message) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
        sqlx::query(
            "INSERT INTO users (id, username, password_hash, created_at) VALUES ($1, $2, 'x', $3)",
        )
        .bind(user)
        .bind(format!("tg302-{}", user.simple()))
        .bind(now)
        .execute($pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO chats (id, title, password_hash, creator_user_id, join_policy, \
             created_at) VALUES ($1, $2, '', $3, 'open', $4)",
        )
        .bind(chat)
        .bind(format!("tg302-chat-{}", chat.simple()))
        .bind(user)
        .bind(now)
        .execute($pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO messages (id, room_id, sender_id, sender, content, created_at) \
             VALUES ($1, $2, $3, 'tg302', 'legacy', $4)",
        )
        .bind(message)
        .bind(chat)
        .bind(user)
        .bind(now)
        .execute($pool)
        .await
        .unwrap();
        (user, chat, message)
    }};
}

/// The schema rules the sticker module relies on, in plain SQL for both pools: a legacy
/// message stays unclassified, a sticker message points at its sticker, short names are
/// unique, and deleting a set cascades to its stickers and nulls the message reference.
macro_rules! assert_sticker_semantics {
    ($pool:expr, $seeded:expr) => {{
        let (user, chat, legacy) = $seeded;
        let now = Utc::now();
        let kind: Option<String> =
            sqlx::query_scalar("SELECT media_kind FROM messages WHERE id = $1")
                .bind(legacy)
                .fetch_one($pool)
                .await
                .unwrap();
        assert_eq!(kind, None, "existing messages stay unclassified");

        let (set, sticker, message) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
        let short_name = format!("tg302_{}", set.simple());
        let insert_set = "INSERT INTO sticker_sets \
             (id, short_name, title, owner_id, created_at, updated_at) \
             VALUES ($1, $2, 'Set', $3, $4, $4)";
        sqlx::query(insert_set)
            .bind(set)
            .bind(&short_name)
            .bind(user)
            .bind(now)
            .execute($pool)
            .await
            .unwrap();
        let duplicate = sqlx::query(insert_set)
            .bind(Uuid::new_v4())
            .bind(&short_name)
            .bind(user)
            .bind(now)
            .execute($pool)
            .await;
        assert!(duplicate.is_err(), "short_name must be unique");
        let set_type: String =
            sqlx::query_scalar("SELECT set_type FROM sticker_sets WHERE id = $1")
                .bind(set)
                .fetch_one($pool)
                .await
                .unwrap();
        assert_eq!(set_type, "regular");
        sqlx::query(
            "INSERT INTO stickers (id, set_id, position, emoji, format, mime_type, width, \
             height, duration_ms, size_bytes, content_hash, storage_key, access_key, created_at) \
             VALUES ($1, $2, 0, '🦀', 'tgs', 'application/x-tgsticker', 512, 512, 2000, 10, \
             'hash', 'hash', $3, $4)",
        )
        .bind(sticker)
        .bind(set)
        .bind(Uuid::new_v4())
        .bind(now)
        .execute($pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO messages (id, room_id, sender_id, sender, content, media_kind, \
             sticker_id, created_at) VALUES ($1, $2, $3, 'tg302', '', 'sticker', $4, $5)",
        )
        .bind(message)
        .bind(chat)
        .bind(user)
        .bind(sticker)
        .bind(now)
        .execute($pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO user_sticker_sets (user_id, set_id, position, installed_at) \
             VALUES ($1, $2, 0, $3)",
        )
        .bind(user)
        .bind(set)
        .bind(now)
        .execute($pool)
        .await
        .unwrap();

        sqlx::query("DELETE FROM sticker_sets WHERE id = $1")
            .bind(set)
            .execute($pool)
            .await
            .unwrap();
        let orphaned: Option<Uuid> =
            sqlx::query_scalar("SELECT sticker_id FROM messages WHERE id = $1")
                .bind(message)
                .fetch_one($pool)
                .await
                .unwrap();
        assert_eq!(
            orphaned, None,
            "deleting the sticker must SET NULL on the message"
        );
        let remaining: i64 = sqlx::query_scalar(
            "SELECT (SELECT COUNT(*) FROM stickers WHERE set_id = $1) + \
             (SELECT COUNT(*) FROM user_sticker_sets WHERE set_id = $1)",
        )
        .bind(set)
        .fetch_one($pool)
        .await
        .unwrap();
        assert_eq!(
            remaining, 0,
            "deleting a set cascades to stickers and libraries"
        );
    }};
}

async fn assert_sqlite_tables(pool: &sqlx::SqlitePool) {
    for table in TABLES {
        let exists: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = $1",
        )
        .bind(table)
        .fetch_one(pool)
        .await
        .unwrap();
        assert_eq!(exists, 1, "{table} missing");
    }
    let columns: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM pragma_table_info('messages') \
         WHERE name IN ('media_kind', 'sticker_id')",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(columns, 2);
}

async fn assert_postgres_tables(pool: &sqlx::PgPool) {
    for table in TABLES {
        let exists: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM information_schema.tables \
             WHERE table_schema = 'public' AND table_name = $1",
        )
        .bind(table)
        .fetch_one(pool)
        .await
        .unwrap();
        assert_eq!(exists, 1, "{table} missing");
    }
    let columns: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM information_schema.columns WHERE table_schema = 'public' \
         AND table_name = 'messages' AND column_name IN ('media_kind', 'sticker_id')",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(columns, 2);
}

#[tokio::test]
async fn sqlite_fresh_schema_has_sticker_tables() {
    let database = sqlite_scratch_path("stickers-fresh");
    let state = AppState::open(&database).await.unwrap();
    assert_sqlite_tables(state.pool()).await;
    let seeded = seed_message!(state.pool());
    assert_sticker_semantics!(state.pool(), seeded);
    state.pool().close().await;
    remove_sqlite_files(&database);
}

#[tokio::test]
async fn sqlite_upgrade_keeps_existing_messages_unclassified() {
    let database = sqlite_scratch_path("stickers-upgrade");
    let pool = sqlite_pool(&database).await;
    let directory = migrations_path("migrations");
    migrations_through(&directory, BEFORE_TG_302)
        .await
        .run(&pool)
        .await
        .unwrap();
    let seeded = seed_message!(&pool);
    pool.close().await;

    let state = AppState::open(&database).await.unwrap();
    let applied: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM _sqlx_migrations")
        .fetch_one(state.pool())
        .await
        .unwrap();
    assert_eq!(applied, migration_count(&directory).await);
    assert_sqlite_tables(state.pool()).await;
    assert_sticker_semantics!(state.pool(), seeded);
    state.pool().close().await;
    remove_sqlite_files(&database);
}

#[tokio::test]
async fn postgres_fresh_schema_has_sticker_tables() {
    let Some((admin_url, admin_pool)) =
        postgres_admin_pool("postgres_fresh_schema_has_sticker_tables").await
    else {
        return;
    };
    let scratch = create_postgres_scratch(&admin_url, &admin_pool, "stickers_fresh").await;
    let state = AppState::open_postgres(&scratch.url, &AppConfig::default())
        .await
        .unwrap();
    let postgres = state.postgres_pool().unwrap();
    assert_postgres_tables(postgres).await;
    let seeded = seed_message!(postgres);
    assert_sticker_semantics!(postgres, seeded);
    postgres.close().await;
    drop(state);
    drop_postgres_scratch(&admin_pool, &scratch).await;
}

#[tokio::test]
async fn postgres_upgrade_keeps_existing_messages_unclassified() {
    let Some((admin_url, admin_pool)) =
        postgres_admin_pool("postgres_upgrade_keeps_existing_messages_unclassified").await
    else {
        return;
    };
    let scratch = create_postgres_scratch(&admin_url, &admin_pool, "stickers_upgrade").await;
    let pool = postgres_pool(&scratch.url).await;
    let directory = migrations_path("migrations-postgres");
    migrations_through(&directory, BEFORE_TG_302)
        .await
        .run(&pool)
        .await
        .unwrap();
    let seeded = seed_message!(&pool);
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
    assert_postgres_tables(postgres).await;
    assert_sticker_semantics!(postgres, seeded);
    postgres.close().await;
    drop(state);
    drop_postgres_scratch(&admin_pool, &scratch).await;
}

#[tokio::test]
async fn sticker_migrations_exist_in_both_adapters_under_the_reserved_versions() {
    for directory in ["migrations", "migrations-postgres"] {
        let migrator = Migrator::new(migrations_path(directory).as_path())
            .await
            .unwrap();
        for (version, description) in [
            (20261201000001, "add sticker tables"),
            (20261201000002, "add message media kind"),
        ] {
            assert!(
                migrator
                    .iter()
                    .any(|m| m.version == version && m.description == description),
                "{directory} is missing {version} {description}"
            );
        }
    }
}
