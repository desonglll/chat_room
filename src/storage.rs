//! SQLite connection setup and embedded schema migrations.

use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result};
use sqlx::postgres::PgPoolOptions;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::{PgPool, SqlitePool};
use uuid::Uuid;

use crate::attachment_storage::AttachmentStore;

pub enum DatabasePool {
    Sqlite(SqlitePool),
    Postgres(PgPool),
}

/// Open or create a SQLite database and apply all embedded migrations.
pub async fn open_database(
    database_path: &Path,
    attachment_store: &AttachmentStore,
) -> Result<SqlitePool> {
    if let Some(parent) = database_path.parent() {
        if !parent.as_os_str().is_empty() {
            tokio::fs::create_dir_all(parent)
                .await
                .with_context(|| format!("create database directory {}", parent.display()))?;
        }
    }

    let options = SqliteConnectOptions::new()
        .filename(database_path)
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal)
        .busy_timeout(Duration::from_secs(5));

    let pool = SqlitePoolOptions::new()
        .max_connections(3)
        .connect_with(options)
        .await
        .with_context(|| format!("open SQLite database {}", database_path.display()))?;

    export_legacy_attachments(&pool, attachment_store).await?;
    run_migrations(&pool).await?;
    Ok(pool)
}

pub async fn open_postgres_database(url: &str, max_connections: u32) -> Result<DatabasePool> {
    let pool = PgPoolOptions::new()
        .max_connections(max_connections)
        .connect(url)
        .await
        .context("open PostgreSQL database")?;
    sqlx::migrate!("./migrations-postgres")
        .run(&pool)
        .await
        .context("run PostgreSQL migrations")?;
    Ok(DatabasePool::Postgres(pool))
}

/// Create a one-connection in-memory database for focused tests.
pub async fn open_memory_database(attachment_store: &AttachmentStore) -> Result<SqlitePool> {
    let options = SqliteConnectOptions::new()
        .filename(":memory:")
        .foreign_keys(true)
        .busy_timeout(Duration::from_secs(5));

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .context("open in-memory SQLite database")?;

    export_legacy_attachments(&pool, attachment_store).await?;
    run_migrations(&pool).await?;
    Ok(pool)
}

async fn export_legacy_attachments(
    pool: &SqlitePool,
    attachment_store: &AttachmentStore,
) -> Result<()> {
    let has_data_column: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM pragma_table_info('attachments') WHERE name = 'data')",
    )
    .fetch_one(pool)
    .await
    .context("inspect legacy attachment schema")?;
    if !has_data_column {
        return Ok(());
    }

    let rows: Vec<(Uuid, Vec<u8>)> = sqlx::query_as("SELECT id, data FROM attachments")
        .fetch_all(pool)
        .await
        .context("load legacy attachment blobs")?;
    for (id, data) in rows {
        attachment_store
            .import_legacy(id, &data)
            .await
            .with_context(|| format!("export legacy attachment {id}"))?;
    }
    Ok(())
}

async fn run_migrations(pool: &SqlitePool) -> Result<()> {
    sqlx::migrate!("./migrations")
        .run(pool)
        .await
        .context("run SQLite migrations")
}
