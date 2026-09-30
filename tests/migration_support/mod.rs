//! Scratch-database plumbing shared by the migration tests.
//!
//! Every helper here is about *getting a database into a known migration state*. What is then
//! asserted about the schema lives in `chat_schema`.

#![allow(dead_code)]

use std::{
    borrow::Cow,
    path::{Path, PathBuf},
};

use sqlx::{
    migrate::Migrator,
    postgres::PgPoolOptions,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
    PgPool, SqlitePool,
};

pub mod chat_data;
pub mod chat_schema;
#[path = "../service_skip/mod.rs"]
mod service_skip;

/// The newest migration that existed before TG-004 — i.e. the schema at `a16f422`, the commit
/// the Telegram-parity programme started from, and therefore the version every deployment is
/// upgrading *from*.
pub const PRE_TG_004_VERSION: i64 = 20260901060000;

pub async fn migrations_through(directory: &Path, version: i64) -> Migrator {
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

pub fn migrations_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(name)
}

pub fn remove_sqlite_files(path: &Path) {
    let _ = std::fs::remove_file(path);
    let _ = std::fs::remove_file(format!("{}-wal", path.display()));
    let _ = std::fs::remove_file(format!("{}-shm", path.display()));
}

pub fn sqlite_scratch_path(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!("chat-room-{label}-{}.db", uuid::Uuid::new_v4()))
}

/// `foreign_keys(true)` mirrors `src/storage.rs`. It is not decoration: SQLite only rewrites
/// the `REFERENCES` clauses of *other* tables during `ALTER TABLE ... RENAME TO` when foreign
/// keys are enabled, and the cascade assertions in `chat_schema` need them enforced.
pub async fn sqlite_pool(path: &Path) -> SqlitePool {
    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true)
        .foreign_keys(true);
    SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .unwrap()
}

/// `TEST_POSTGRES_ADMIN_URL` set means "PostgreSQL is required": the helper panics instead of
/// skipping, so a broken PostgreSQL migration cannot hide behind a green run. Unset, the test
/// skips — visibly, through `service_skip`'s marker on the real stderr — which is what an
/// environment without Docker needs.
pub async fn postgres_admin_pool(test_name: &str) -> Option<(String, PgPool)> {
    service_skip::postgres_admin_pool_or_skip(test_name).await
}

pub struct PostgresScratch {
    pub name: String,
    pub url: String,
}

pub async fn create_postgres_scratch(
    admin_url: &str,
    admin_pool: &PgPool,
    label: &str,
) -> PostgresScratch {
    let name = format!("chat_room_{label}_{}", uuid::Uuid::new_v4().simple());
    sqlx::query(&format!(r#"CREATE DATABASE "{name}""#))
        .execute(admin_pool)
        .await
        .unwrap();
    let base = admin_url.rsplit_once('/').unwrap().0;
    let url = format!("{base}/{name}");
    PostgresScratch { name, url }
}

pub async fn drop_postgres_scratch(admin_pool: &PgPool, scratch: &PostgresScratch) {
    sqlx::query("SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE datname = $1")
        .bind(&scratch.name)
        .execute(admin_pool)
        .await
        .ok();
    sqlx::query(&format!(r#"DROP DATABASE "{}""#, scratch.name))
        .execute(admin_pool)
        .await
        .unwrap();
}

pub async fn postgres_pool(url: &str) -> PgPool {
    PgPoolOptions::new()
        .max_connections(1)
        .connect(url)
        .await
        .unwrap()
}

/// How many migrations the directory holds in total, for the "every migration applied" check.
pub async fn migration_count(directory: &Path) -> i64 {
    Migrator::new(directory).await.unwrap().iter().count() as i64
}
