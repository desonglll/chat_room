//! TG-404: `20270101000004_add_scheduled_messages` on both adapters, fresh and upgraded from
//! the pre-TG-004 baseline. Checked semantics: the `scheduled_messages` table and its cascade
//! from `chats`, `messages.silent` defaulting to false for existing and new rows, and the
//! rewritten reply / mention notification triggers skipping silent messages only.

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

macro_rules! assert_scheduled_semantics {
    ($pool:expr) => {{
        let now = Utc::now();
        let (alice, bob, chat) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
        for user in [alice, bob] {
            sqlx::query(
                "INSERT INTO users (id, username, password_hash, created_at) \
                 VALUES ($1, $2, 'x', $3)",
            )
            .bind(user)
            .bind(format!("tg404-{}", user.simple()))
            .bind(now)
            .execute($pool)
            .await
            .unwrap();
        }
        sqlx::query(
            "INSERT INTO chats (id, title, password_hash, creator_user_id, join_policy, \
             created_at) VALUES ($1, $2, '', $3, 'open', $4)",
        )
        .bind(chat)
        .bind(format!("tg404-chat-{}", chat.simple()))
        .bind(alice)
        .bind(now)
        .execute($pool)
        .await
        .unwrap();
        let message = |id: Uuid, sender: Uuid, reply_to: Option<Uuid>, silent: Option<bool>| {
            let query = match silent {
                Some(_) => "INSERT INTO messages (id, room_id, sender_id, sender, content, \
                            reply_to_id, created_at, silent) VALUES ($1, $2, $3, 's', 'c', $4, $5, $6)",
                None => "INSERT INTO messages (id, room_id, sender_id, sender, content, \
                         reply_to_id, created_at) VALUES ($1, $2, $3, 's', 'c', $4, $5)",
            };
            let mut query = sqlx::query(query)
                .bind(id)
                .bind(chat)
                .bind(sender)
                .bind(reply_to)
                .bind(now);
            if let Some(silent) = silent {
                query = query.bind(silent);
            }
            query
        };
        let notifications = |message_id: Uuid| {
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM notifications WHERE message_id = $1",
            )
            .bind(message_id)
        };

        let source = Uuid::new_v4();
        message(source, bob, None, None).execute($pool).await.unwrap();
        let silent: bool = sqlx::query_scalar("SELECT silent FROM messages WHERE id = $1")
            .bind(source)
            .fetch_one($pool)
            .await
            .unwrap();
        assert!(!silent, "silent defaults to false");

        let (loud, quiet) = (Uuid::new_v4(), Uuid::new_v4());
        message(loud, alice, Some(source), None).execute($pool).await.unwrap();
        message(quiet, alice, Some(source), Some(true)).execute($pool).await.unwrap();
        assert_eq!(notifications(loud).fetch_one($pool).await.unwrap(), 1, "reply notifies");
        assert_eq!(notifications(quiet).fetch_one($pool).await.unwrap(), 0, "silent reply");

        for id in [loud, quiet] {
            sqlx::query(
                "INSERT INTO message_mentions (message_id, mentioned_user_id, created_at) \
                 VALUES ($1, $2, $3)",
            )
            .bind(id)
            .bind(bob)
            .bind(now)
            .execute($pool)
            .await
            .unwrap();
        }
        assert_eq!(notifications(loud).fetch_one($pool).await.unwrap(), 2, "+ mention");
        assert_eq!(notifications(quiet).fetch_one($pool).await.unwrap(), 0, "silent mention");

        let scheduled = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO scheduled_messages (id, room_id, sender_id, content, reply_to_id, \
             silent, scheduled_at, created_at, updated_at) \
             VALUES ($1, $2, $3, 'later', $4, TRUE, $5, $5, $5)",
        )
        .bind(scheduled)
        .bind(chat)
        .bind(alice)
        .bind(source)
        .bind(now)
        .execute($pool)
        .await
        .unwrap();
        let entities: String =
            sqlx::query_scalar("SELECT entities FROM scheduled_messages WHERE id = $1")
                .bind(scheduled)
                .fetch_one($pool)
                .await
                .unwrap();
        assert_eq!(entities, "[]");
        let in_messages: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM messages WHERE id = $1")
            .bind(scheduled)
            .fetch_one($pool)
            .await
            .unwrap();
        assert_eq!(in_messages, 0, "a scheduled row is not a message");

        sqlx::query("DELETE FROM chats WHERE id = $1")
            .bind(chat)
            .execute($pool)
            .await
            .unwrap();
        let left: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM scheduled_messages WHERE id = $1")
                .bind(scheduled)
                .fetch_one($pool)
                .await
                .unwrap();
        assert_eq!(left, 0, "deleting the chat cascades to its scheduled messages");
    }};
}

macro_rules! applied_count {
    ($pool:expr) => {
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM _sqlx_migrations")
            .fetch_one($pool)
            .await
            .unwrap()
    };
}

#[tokio::test]
async fn sqlite_fresh_schema_has_scheduled_messages() {
    let database = sqlite_scratch_path("scheduled-fresh");
    let state = AppState::open(&database).await.unwrap();
    assert_eq!(
        applied_count!(state.pool()),
        migration_count(&migrations_path("migrations")).await
    );
    assert_scheduled_semantics!(state.pool());
    state.pool().close().await;
    remove_sqlite_files(&database);
}

#[tokio::test]
async fn sqlite_upgrade_from_the_pre_tg_004_schema_gains_scheduled_messages() {
    let database = sqlite_scratch_path("scheduled-upgrade");
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
    assert_eq!(
        applied_count!(state.pool()),
        migration_count(&directory).await
    );
    let unset: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM messages WHERE silent")
        .fetch_one(state.pool())
        .await
        .unwrap();
    assert_eq!(unset, 0, "existing messages are not silent");
    assert_scheduled_semantics!(state.pool());
    state.pool().close().await;
    remove_sqlite_files(&database);
}

#[tokio::test]
async fn postgres_fresh_schema_has_scheduled_messages() {
    let Some((admin_url, admin_pool)) =
        postgres_admin_pool("postgres_fresh_schema_has_scheduled_messages").await
    else {
        return;
    };
    let scratch = create_postgres_scratch(&admin_url, &admin_pool, "scheduled_fresh").await;
    let state = AppState::open_postgres(&scratch.url, &AppConfig::default())
        .await
        .unwrap();
    let postgres = state.postgres_pool().unwrap();
    assert_eq!(
        applied_count!(postgres),
        migration_count(&migrations_path("migrations-postgres")).await
    );
    assert_scheduled_semantics!(postgres);
    postgres.close().await;
    drop(state);
    drop_postgres_scratch(&admin_pool, &scratch).await;
}

#[tokio::test]
async fn postgres_upgrade_from_the_pre_tg_004_schema_gains_scheduled_messages() {
    let Some((admin_url, admin_pool)) =
        postgres_admin_pool("postgres_upgrade_from_the_pre_tg_004_schema_gains_scheduled_messages")
            .await
    else {
        return;
    };
    let scratch = create_postgres_scratch(&admin_url, &admin_pool, "scheduled_upgrade").await;
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
    assert_eq!(applied_count!(postgres), migration_count(&directory).await);
    let unset: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM messages WHERE silent")
        .fetch_one(postgres)
        .await
        .unwrap();
    assert_eq!(unset, 0, "existing messages are not silent");
    assert_scheduled_semantics!(postgres);
    postgres.close().await;
    drop(state);
    drop_postgres_scratch(&admin_pool, &scratch).await;
}

#[tokio::test]
async fn the_scheduled_migration_exists_in_both_adapters_under_the_reserved_version() {
    for directory in ["migrations", "migrations-postgres"] {
        let migrator = Migrator::new(migrations_path(directory).as_path())
            .await
            .unwrap();
        assert!(
            migrator
                .iter()
                .any(|migration| migration.version == 20270101000004
                    && migration.description == "add scheduled messages"),
            "{directory} is missing 20270101000004_add_scheduled_messages"
        );
    }
}
