//! PostgreSQL and local-attachment backup packages.

mod archive;
mod automation;
mod automation_store;
mod files;
mod package;
mod postgres;
mod sqlite;

use std::{fs, path::PathBuf};

use anyhow::{Context, Result};
use uuid::Uuid;

use crate::config::AppConfig;

pub use archive::{pack_archive, unpack_archive};
pub use automation::{ensure_scheduler, export_state, run_backup};
pub use automation_store::{status, BackupRun, BackupStatus};
pub use package::{read_and_verify, BackupFile, BackupManifest};
pub use postgres::{export_postgres, export_postgres_scoped, restore_postgres};
pub use sqlite::{export_sqlite_scoped, restore_sqlite};

pub const ARCHIVE_CONTENT_TYPE: &str = "application/gzip";
pub const ARCHIVE_EXTENSION: &str = "tar.gz";

#[derive(Debug)]
pub struct RestoreOutcome {
    pub previous_database: Option<PathBuf>,
    pub previous_attachments: Option<PathBuf>,
    pub redis_keys_cleared: usize,
    pub includes_files: bool,
}

/// A unique writable directory beside the local attachment objects.
pub fn create_work_directory(config: &AppConfig, label: &str) -> Result<PathBuf> {
    let root = config
        .attachments
        .directory
        .join(".backup-work")
        .join(format!("{label}-{}", Uuid::new_v4().simple()));
    fs::create_dir_all(&root)
        .with_context(|| format!("create backup work directory {}", root.display()))?;
    Ok(root)
}

pub fn remove_work_directory(path: &std::path::Path) {
    if let Err(error) = fs::remove_dir_all(path) {
        if error.kind() != std::io::ErrorKind::NotFound {
            tracing::warn!(path = %path.display(), "remove backup work directory failed: {error}");
        }
    }
}
