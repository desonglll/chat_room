//! TG-406: the poll tables (`20270101000006`) on both adapters, fresh and upgraded.
//!
//! Fresh path: `AppState::open`/`open_postgres` on an empty scratch database. Upgrade path: the
//! pre-TG-004 schema with seeded rows, carried through every later migration. Both then check
//! the schema semantics the poll module relies on: cascade from `messages`, the quiz CHECK,
//! the composite (poll, option) foreign key, and one row per (poll, voter, option).

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

macro_rules! assert_poll_semantics {
    ($pool:expr) => {{
        let now = Utc::now();
        let (user, chat, message) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
        sqlx::query(
            "INSERT INTO users (id, username, password_hash, created_at) VALUES ($1, $2, 'x', $3)",
        )
        .bind(user)
        .bind(format!("tg406-{}", user.simple()))
        .bind(now)
        .execute($pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO chats (id, title, password_hash, creator_user_id, join_policy, \
             created_at) VALUES ($1, $2, '', $3, 'open', $4)",
        )
        .bind(chat)
        .bind(format!("tg406-chat-{}", chat.simple()))
        .bind(user)
        .bind(now)
        .execute($pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO messages (id, room_id, sender_id, sender, content, created_at) \
             VALUES ($1, $2, $3, 'tg406', 'Q?', $4)",
        )
        .bind(message)
        .bind(chat)
        .bind(user)
        .bind(now)
        .execute($pool)
        .await
        .unwrap();

        let quiz_without_answer = sqlx::query(
            "INSERT INTO polls (message_id, room_id, creator_id, question, quiz, created_at) \
             VALUES ($1, $2, $3, 'Q?', TRUE, $4)",
        )
        .bind(message)
        .bind(chat)
        .bind(user)
        .bind(now)
        .execute($pool)
        .await;
        assert!(
            quiz_without_answer.is_err(),
            "a quiz needs a correct option"
        );

        sqlx::query(
            "INSERT INTO polls (message_id, room_id, creator_id, question, quiz, correct_option, \
             created_at) VALUES ($1, $2, $3, 'Q?', TRUE, 0, $4)",
        )
        .bind(message)
        .bind(chat)
        .bind(user)
        .bind(now)
        .execute($pool)
        .await
        .unwrap();
        for position in 0..2_i32 {
            sqlx::query(
                "INSERT INTO poll_options (message_id, position, text) VALUES ($1, $2, 'o')",
            )
            .bind(message)
            .bind(position)
            .execute($pool)
            .await
            .unwrap();
        }
        let vote = |position: i32| {
            sqlx::query(
                "INSERT INTO poll_votes (message_id, position, user_id, voted_at) \
                 VALUES ($1, $2, $3, $4)",
            )
            .bind(message)
            .bind(position)
            .bind(user)
            .bind(now)
        };
        vote(0).execute($pool).await.unwrap();
        assert!(
            vote(0).execute($pool).await.is_err(),
            "one row per (poll, voter, option)"
        );
        assert!(
            vote(7).execute($pool).await.is_err(),
            "the option must exist"
        );

        sqlx::query("DELETE FROM messages WHERE id = $1")
            .bind(message)
            .execute($pool)
            .await
            .unwrap();
        for table in ["polls", "poll_options", "poll_votes"] {
            let left: i64 = sqlx::query_scalar(&format!(
                "SELECT COUNT(*) FROM {table} WHERE message_id = $1"
            ))
            .bind(message)
            .fetch_one($pool)
            .await
            .unwrap();
            assert_eq!(left, 0, "deleting the message must cascade to {table}");
        }
    }};
}

#[tokio::test]
async fn sqlite_fresh_schema_has_polls() {
    let database = sqlite_scratch_path("polls-fresh");
    let state = AppState::open(&database).await.unwrap();
    let applied: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM _sqlx_migrations")
        .fetch_one(state.pool())
        .await
        .unwrap();
    assert_eq!(
        applied,
        migration_count(&migrations_path("migrations")).await
    );
    assert_poll_semantics!(state.pool());
    state.pool().close().await;
    remove_sqlite_files(&database);
}

#[tokio::test]
async fn sqlite_upgrade_from_the_pre_tg_004_schema_gains_polls() {
    let database = sqlite_scratch_path("polls-upgrade");
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
    assert_poll_semantics!(state.pool());
    state.pool().close().await;
    remove_sqlite_files(&database);
}

#[tokio::test]
async fn postgres_fresh_schema_has_polls() {
    let Some((admin_url, admin_pool)) =
        postgres_admin_pool("postgres_fresh_schema_has_polls").await
    else {
        return;
    };
    let scratch = create_postgres_scratch(&admin_url, &admin_pool, "polls_fresh").await;
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
    assert_poll_semantics!(postgres);
    postgres.close().await;
    drop(state);
    drop_postgres_scratch(&admin_pool, &scratch).await;
}

#[tokio::test]
async fn postgres_upgrade_from_the_pre_tg_004_schema_gains_polls() {
    let Some((admin_url, admin_pool)) =
        postgres_admin_pool("postgres_upgrade_from_the_pre_tg_004_schema_gains_polls").await
    else {
        return;
    };
    let scratch = create_postgres_scratch(&admin_url, &admin_pool, "polls_upgrade").await;
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
    assert_poll_semantics!(postgres);
    postgres.close().await;
    drop(state);
    drop_postgres_scratch(&admin_pool, &scratch).await;
}

#[tokio::test]
async fn the_poll_migration_exists_in_both_adapters_under_the_reserved_version() {
    for directory in ["migrations", "migrations-postgres"] {
        let migrator = Migrator::new(migrations_path(directory).as_path())
            .await
            .unwrap();
        assert!(
            migrator
                .iter()
                .any(|migration| migration.version == 20270101000006
                    && migration.description == "add polls"),
            "{directory} is missing 20270101000006_add_polls"
        );
    }
}
