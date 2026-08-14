//! SQLite setup, migrations, and legacy JSON import.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::SqlitePool;
use uuid::Uuid;

const LEGACY_IMPORT_KEY: &str = "legacy_json_import_v1";

#[derive(Deserialize)]
struct LegacyRoom {
    id: Uuid,
    name: String,
    #[serde(default)]
    password_hash: String,
    #[serde(default)]
    has_password: Option<bool>,
    created_at: DateTime<Utc>,
}

/// Open or create a SQLite database, run schema migrations, and import an
/// existing JSON store once when one is present.
pub async fn open_database(
    database_path: &Path,
    legacy_json_path: Option<&Path>,
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

    run_migrations(&pool).await?;
    if let Some(path) = legacy_json_path {
        import_legacy_json(&pool, path).await?;
    }

    Ok(pool)
}

/// Create a one-connection in-memory database for focused tests.
pub async fn open_memory_database() -> Result<SqlitePool> {
    let options = SqliteConnectOptions::new()
        .filename(":memory:")
        .foreign_keys(true)
        .busy_timeout(Duration::from_secs(5));

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .context("open in-memory SQLite database")?;

    run_migrations(&pool).await?;
    Ok(pool)
}

async fn run_migrations(pool: &SqlitePool) -> Result<()> {
    sqlx::migrate!("./migrations")
        .run(pool)
        .await
        .context("run SQLite migrations")
}

async fn import_legacy_json(pool: &SqlitePool, path: &Path) -> Result<()> {
    let already_imported: Option<String> =
        sqlx::query_scalar("SELECT value FROM app_metadata WHERE key = ?")
            .bind(LEGACY_IMPORT_KEY)
            .fetch_optional(pool)
            .await
            .context("check legacy import marker")?;

    if already_imported.is_some() || !path.exists() {
        return Ok(());
    }

    let data = match tokio::fs::read_to_string(path).await {
        Ok(data) => data,
        Err(error) => {
            tracing::warn!("cannot read legacy room file {}: {}", path.display(), error);
            return Ok(());
        }
    };

    let rooms: HashMap<String, LegacyRoom> = match serde_json::from_str(&data) {
        Ok(rooms) => rooms,
        Err(error) => {
            tracing::warn!(
                "cannot parse legacy room file {}: {}",
                path.display(),
                error
            );
            return Ok(());
        }
    };

    let mut tx = pool.begin().await.context("start legacy import")?;
    let mut imported = 0_u64;
    let mut skipped = 0_u64;

    for room in rooms.into_values() {
        let has_password = match room.has_password {
            Some(value) => value,
            None if !room.password_hash.is_empty() => true,
            None => {
                skipped += 1;
                tracing::warn!(
                    "skipping legacy room '{}' ({}): privacy state is unknown",
                    room.name,
                    room.id
                );
                continue;
            }
        };

        if has_password && room.password_hash.is_empty() {
            skipped += 1;
            tracing::warn!(
                "skipping legacy private room '{}' ({}): password hash was not stored",
                room.name,
                room.id
            );
            continue;
        }

        let password_hash = if has_password {
            room.password_hash
        } else {
            String::new()
        };

        let result = sqlx::query(
            "INSERT OR IGNORE INTO rooms (id, name, password_hash, created_at) \
             VALUES (?, ?, ?, ?)",
        )
        .bind(room.id)
        .bind(room.name)
        .bind(password_hash)
        .bind(room.created_at)
        .execute(&mut *tx)
        .await
        .context("insert legacy room")?;

        imported += result.rows_affected();
    }

    let summary = format!("imported={imported};skipped={skipped}");
    sqlx::query(
        "INSERT INTO app_metadata (key, value) VALUES (?, ?) \
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
    )
    .bind(LEGACY_IMPORT_KEY)
    .bind(&summary)
    .execute(&mut *tx)
    .await
    .context("record legacy import marker")?;

    tx.commit().await.context("commit legacy import")?;

    let backup = legacy_backup_path(path);
    if !backup.exists() {
        if let Err(error) = tokio::fs::copy(path, &backup).await {
            tracing::warn!(
                "could not back up legacy file {} to {}: {}",
                path.display(),
                backup.display(),
                error
            );
        }
    }

    tracing::info!(
        "legacy room import complete from {}: {}",
        path.display(),
        summary
    );
    Ok(())
}

fn legacy_backup_path(path: &Path) -> PathBuf {
    PathBuf::from(format!("{}.backup", path.display()))
}
