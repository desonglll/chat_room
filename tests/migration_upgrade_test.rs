//! Upgrade-path coverage for the paired FND migrations on SQLite and PostgreSQL.

use std::{borrow::Cow, path::Path};

use chat_room::{config::AppConfig, state::AppState};
use sqlx::{
    migrate::Migrator,
    postgres::PgPoolOptions,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
};

const PRE_FND_002_VERSION: i64 = 20260826000003;

async fn migrations_through(directory: &Path, version: i64) -> Migrator {
    let all = Migrator::new(directory).await.unwrap();
    let migrations = all
        .iter()
        .filter(|migration| migration.version <= version)
        .cloned()
        .collect();
    Migrator {
        migrations: Cow::Owned(migrations),
        ..Migrator::DEFAULT
    }
}

fn migrations_path(name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(name)
}

fn remove_sqlite_files(path: &Path) {
    let _ = std::fs::remove_file(path);
    let _ = std::fs::remove_file(format!("{}-wal", path.display()));
    let _ = std::fs::remove_file(format!("{}-shm", path.display()));
}

#[tokio::test]
async fn sqlite_upgrades_from_the_pre_fnd_002_schema() {
    let database = std::env::temp_dir().join(format!(
        "chat-room-sqlite-upgrade-{}.db",
        uuid::Uuid::new_v4()
    ));
    let options = SqliteConnectOptions::new()
        .filename(&database)
        .create_if_missing(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .unwrap();
    let directory = migrations_path("migrations");
    let old = migrations_through(&directory, PRE_FND_002_VERSION).await;
    old.run(&pool).await.unwrap();
    let old_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM _sqlx_migrations")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(old_count, old.iter().count() as i64);
    pool.close().await;

    let state = AppState::open(&database).await.unwrap();
    let full = Migrator::new(directory.as_path()).await.unwrap();
    let upgraded_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM _sqlx_migrations")
        .fetch_one(state.pool())
        .await
        .unwrap();
    assert_eq!(upgraded_count, full.iter().count() as i64);
    let ai_columns: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM pragma_table_info('ai_thread_messages') \
         WHERE name IN ('stage', 'stage_started_at', 'trace')",
    )
    .fetch_one(state.pool())
    .await
    .unwrap();
    assert_eq!(ai_columns, 3);
    let catch_up_columns: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM pragma_table_info('ai_runs') \
         WHERE name IN ('purpose', 'source_after_message_id', \
           'source_through_message_id', 'source_message_count')",
    )
    .fetch_one(state.pool())
    .await
    .unwrap();
    assert_eq!(catch_up_columns, 4);
    let room_pins: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'room_pins'",
    )
    .fetch_one(state.pool())
    .await
    .unwrap();
    assert_eq!(room_pins, 1);
    let room_tasks: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'room_tasks'",
    )
    .fetch_one(state.pool())
    .await
    .unwrap();
    assert_eq!(room_tasks, 1);
    let extraction_tables: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name IN \
         ('ai_extraction_runs', 'ai_extraction_candidates', \
          'ai_extraction_candidate_sources', 'ai_extraction_run_candidates')",
    )
    .fetch_one(state.pool())
    .await
    .unwrap();
    assert_eq!(extraction_tables, 4);

    state.pool().close().await;
    remove_sqlite_files(&database);
}

fn postgres_admin_url() -> (String, bool) {
    match std::env::var("TEST_POSTGRES_ADMIN_URL") {
        Ok(url) => (url, true),
        Err(_) => (
            "postgresql://postgres:postgres@localhost:52735/postgres".into(),
            false,
        ),
    }
}

async fn postgres_admin_pool() -> Option<(String, sqlx::PgPool)> {
    let (url, required) = postgres_admin_url();
    match PgPoolOptions::new().max_connections(1).connect(&url).await {
        Ok(pool) => Some((url, pool)),
        Err(error) if required => panic!("required PostgreSQL at {url} is unavailable: {error}"),
        Err(error) => {
            eprintln!("skipping PostgreSQL upgrade test: {error}");
            None
        }
    }
}

#[tokio::test]
async fn postgres_upgrades_from_the_pre_fnd_002_schema() {
    let Some((admin_url, admin_pool)) = postgres_admin_pool().await else {
        return;
    };
    let database_name = format!("chat_room_upgrade_{}", uuid::Uuid::new_v4().simple());
    sqlx::query(&format!(r#"CREATE DATABASE "{database_name}""#))
        .execute(&admin_pool)
        .await
        .unwrap();
    let base = admin_url.rsplit_once('/').unwrap().0;
    let database_url = format!("{base}/{database_name}");
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&database_url)
        .await
        .unwrap();
    let directory = migrations_path("migrations-postgres");
    let old = migrations_through(&directory, PRE_FND_002_VERSION).await;
    old.run(&pool).await.unwrap();
    let old_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM _sqlx_migrations")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(old_count, old.iter().count() as i64);
    pool.close().await;

    let state = AppState::open_postgres(&database_url, &AppConfig::default())
        .await
        .unwrap();
    let full = Migrator::new(directory.as_path()).await.unwrap();
    let postgres = state.postgres_pool().unwrap();
    let upgraded_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM _sqlx_migrations")
        .fetch_one(postgres)
        .await
        .unwrap();
    assert_eq!(upgraded_count, full.iter().count() as i64);
    let ai_columns: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM information_schema.columns \
         WHERE table_schema = 'public' AND table_name = 'ai_thread_messages' \
         AND column_name IN ('stage', 'stage_started_at', 'trace')",
    )
    .fetch_one(postgres)
    .await
    .unwrap();
    assert_eq!(ai_columns, 3);
    let catch_up_columns: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM information_schema.columns \
         WHERE table_schema = 'public' AND table_name = 'ai_runs' \
         AND column_name IN ('purpose', 'source_after_message_id', \
           'source_through_message_id', 'source_message_count')",
    )
    .fetch_one(postgres)
    .await
    .unwrap();
    assert_eq!(catch_up_columns, 4);
    let room_pins: Option<String> =
        sqlx::query_scalar("SELECT to_regclass('public.room_pins')::text")
            .fetch_one(postgres)
            .await
            .unwrap();
    assert_eq!(room_pins.as_deref(), Some("room_pins"));
    let room_tasks: Option<String> =
        sqlx::query_scalar("SELECT to_regclass('public.room_tasks')::text")
            .fetch_one(postgres)
            .await
            .unwrap();
    assert_eq!(room_tasks.as_deref(), Some("room_tasks"));
    let extraction_tables: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema = 'public' \
         AND table_name IN ('ai_extraction_runs', 'ai_extraction_candidates', \
          'ai_extraction_candidate_sources', 'ai_extraction_run_candidates')",
    )
    .fetch_one(postgres)
    .await
    .unwrap();
    assert_eq!(extraction_tables, 4);

    postgres.close().await;
    drop(state);
    sqlx::query("SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE datname = $1")
        .bind(&database_name)
        .execute(&admin_pool)
        .await
        .ok();
    sqlx::query(&format!(r#"DROP DATABASE "{database_name}""#))
        .execute(&admin_pool)
        .await
        .unwrap();
}
