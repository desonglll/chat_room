//! TG-304: `message_entities` and `user_emoji_status` (`20261201000003`) on both adapters, on a
//! fresh database and on an upgrade from the schema head just before them with an existing
//! message and custom emoji.

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

/// Everything before this task's reserved version.
const BEFORE_TG_304: i64 = 20261201000002;

/// A user, chat, plain message, and one custom emoji, using only pre-TG-304 tables.
macro_rules! seed {
    ($pool:expr) => {{
        let now = Utc::now();
        let (user, chat, message) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
        let (set, emoji) = (Uuid::new_v4(), Uuid::new_v4());
        sqlx::query(
            "INSERT INTO users (id, username, password_hash, created_at) VALUES ($1, $2, 'x', $3)",
        )
        .bind(user)
        .bind(format!("tg304-{}", user.simple()))
        .bind(now)
        .execute($pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO chats (id, title, password_hash, creator_user_id, join_policy, \
             created_at) VALUES ($1, $2, '', $3, 'open', $4)",
        )
        .bind(chat)
        .bind(format!("tg304-chat-{}", chat.simple()))
        .bind(user)
        .bind(now)
        .execute($pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO messages (id, room_id, sender_id, sender, content, created_at) \
             VALUES ($1, $2, $3, 'tg304', 'legacy 😺', $4)",
        )
        .bind(message)
        .bind(chat)
        .bind(user)
        .bind(now)
        .execute($pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO sticker_sets (id, short_name, title, set_type, owner_id, created_at, \
             updated_at) VALUES ($1, $2, 'Set', 'custom_emoji', $3, $4, $4)",
        )
        .bind(set)
        .bind(format!("tg304_{}", set.simple()))
        .bind(user)
        .bind(now)
        .execute($pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO stickers (id, set_id, position, emoji, format, mime_type, width, \
             height, size_bytes, content_hash, storage_key, access_key, created_at) \
             VALUES ($1, $2, 0, '😺', 'webp', 'image/webp', 100, 100, 10, 'h', 'h', $3, $4)",
        )
        .bind(emoji)
        .bind(set)
        .bind(Uuid::new_v4())
        .bind(now)
        .execute($pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO custom_emoji (sticker_id, set_id, emoji, created_at) \
             VALUES ($1, $2, '😺', $3)",
        )
        .bind(emoji)
        .bind(set)
        .bind(now)
        .execute($pool)
        .await
        .unwrap();
        (user, message, emoji)
    }};
}

/// The schema rules the entity store relies on: an existing message has no entities, an
/// entity row references a custom emoji, deleting the message cascades, hard-deleting the
/// sticker nulls the entity reference and removes the status.
macro_rules! assert_semantics {
    ($pool:expr, $seeded:expr) => {{
        let (user, message, emoji) = $seeded;
        let count = "SELECT COUNT(*) FROM message_entities WHERE message_id = $1";
        let existing: i64 = sqlx::query_scalar(count)
            .bind(message)
            .fetch_one($pool)
            .await
            .unwrap();
        assert_eq!(existing, 0, "existing messages stay plain text");

        let insert = "INSERT INTO message_entities (message_id, position, entity_type, \
             offset_utf16, length_utf16, custom_emoji_id) VALUES ($1, $2, $3, 7, 2, $4)";
        for (position, custom) in [(0_i64, Some(emoji)), (1, None)] {
            sqlx::query(insert)
                .bind(message)
                .bind(position)
                .bind(if custom.is_some() {
                    "custom_emoji"
                } else {
                    "bold"
                })
                .bind(custom)
                .execute($pool)
                .await
                .unwrap();
        }
        let duplicate = sqlx::query(insert)
            .bind(message)
            .bind(0_i64)
            .bind("bold")
            .bind(None::<Uuid>)
            .execute($pool)
            .await;
        assert!(duplicate.is_err(), "(message_id, position) is the key");
        let dangling = sqlx::query(insert)
            .bind(message)
            .bind(2_i64)
            .bind("custom_emoji")
            .bind(Uuid::new_v4())
            .execute($pool)
            .await;
        assert!(
            dangling.is_err(),
            "custom_emoji_id must reference a custom emoji"
        );

        sqlx::query(
            "INSERT INTO user_emoji_status (user_id, custom_emoji_id, expires_at, updated_at) \
             VALUES ($1, $2, NULL, $3)",
        )
        .bind(user)
        .bind(emoji)
        .bind(Utc::now())
        .execute($pool)
        .await
        .unwrap();

        sqlx::query("DELETE FROM stickers WHERE id = $1")
            .bind(emoji)
            .execute($pool)
            .await
            .unwrap();
        let nulled: Option<Uuid> = sqlx::query_scalar(
            "SELECT custom_emoji_id FROM message_entities WHERE message_id = $1 AND position = 0",
        )
        .bind(message)
        .fetch_one($pool)
        .await
        .unwrap();
        assert_eq!(
            nulled, None,
            "a hard-deleted emoji leaves the fallback text"
        );
        let statuses: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM user_emoji_status")
            .fetch_one($pool)
            .await
            .unwrap();
        assert_eq!(statuses, 0, "a status dies with its emoji");

        sqlx::query("DELETE FROM messages WHERE id = $1")
            .bind(message)
            .execute($pool)
            .await
            .unwrap();
        let remaining: i64 = sqlx::query_scalar(count)
            .bind(message)
            .fetch_one($pool)
            .await
            .unwrap();
        assert_eq!(remaining, 0, "entities cascade with their message");
    }};
}

#[tokio::test]
async fn sqlite_fresh_schema_has_entity_tables() {
    let database = sqlite_scratch_path("custom-emoji-fresh");
    let state = AppState::open(&database).await.unwrap();
    let seeded = seed!(state.pool());
    assert_semantics!(state.pool(), seeded);
    state.pool().close().await;
    remove_sqlite_files(&database);
}

#[tokio::test]
async fn sqlite_upgrade_keeps_existing_messages_plain() {
    let database = sqlite_scratch_path("custom-emoji-upgrade");
    let pool = sqlite_pool(&database).await;
    let directory = migrations_path("migrations");
    migrations_through(&directory, BEFORE_TG_304)
        .await
        .run(&pool)
        .await
        .unwrap();
    let seeded = seed!(&pool);
    pool.close().await;

    let state = AppState::open(&database).await.unwrap();
    let applied: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM _sqlx_migrations")
        .fetch_one(state.pool())
        .await
        .unwrap();
    assert_eq!(applied, migration_count(&directory).await);
    assert_semantics!(state.pool(), seeded);
    state.pool().close().await;
    remove_sqlite_files(&database);
}

#[tokio::test]
async fn postgres_fresh_schema_has_entity_tables() {
    let Some((admin_url, admin_pool)) =
        postgres_admin_pool("postgres_fresh_schema_has_entity_tables").await
    else {
        return;
    };
    let scratch = create_postgres_scratch(&admin_url, &admin_pool, "custom_emoji_fresh").await;
    let state = AppState::open_postgres(&scratch.url, &AppConfig::default())
        .await
        .unwrap();
    let postgres = state.postgres_pool().unwrap();
    let seeded = seed!(postgres);
    assert_semantics!(postgres, seeded);
    postgres.close().await;
    drop(state);
    drop_postgres_scratch(&admin_pool, &scratch).await;
}

#[tokio::test]
async fn postgres_upgrade_keeps_existing_messages_plain() {
    let Some((admin_url, admin_pool)) =
        postgres_admin_pool("postgres_upgrade_keeps_existing_messages_plain").await
    else {
        return;
    };
    let scratch = create_postgres_scratch(&admin_url, &admin_pool, "custom_emoji_upgrade").await;
    let pool = postgres_pool(&scratch.url).await;
    let directory = migrations_path("migrations-postgres");
    migrations_through(&directory, BEFORE_TG_304)
        .await
        .run(&pool)
        .await
        .unwrap();
    let seeded = seed!(&pool);
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
    assert_semantics!(postgres, seeded);
    postgres.close().await;
    drop(state);
    drop_postgres_scratch(&admin_pool, &scratch).await;
}

#[tokio::test]
async fn custom_emoji_migration_exists_in_both_adapters_under_the_reserved_version() {
    for directory in ["migrations", "migrations-postgres"] {
        let migrator = Migrator::new(migrations_path(directory).as_path())
            .await
            .unwrap();
        assert!(
            migrator.iter().any(|m| m.version == 20261201000003
                && m.description == "add message entities and emoji status"),
            "{directory} is missing 20261201000003"
        );
    }
}
