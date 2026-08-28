use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::Ordering,
    time::{Duration, Instant},
};

use anyhow::{bail, Context, Result};
use chrono::Utc;
use uuid::Uuid;

use super::{
    automation_store::{insert_run, scheduled_backup_due, BackupRun, RunInsert},
    files::{absolute_normalized, sibling_temp_path},
    package::file_record,
    BackupManifest,
};
use crate::state::{with_pool, AppState, SharedState};

const BACKUP_LOCK_REASON: &str = "consistent backup in progress";

struct WorkCleanup(PathBuf);

impl Drop for WorkCleanup {
    fn drop(&mut self) {
        super::remove_work_directory(&self.0);
    }
}

pub fn ensure_scheduler(state: SharedState) {
    if !state.config.backup.enabled
        || state
            .backup_runtime
            .scheduler_started
            .swap(true, Ordering::AcqRel)
    {
        return;
    }
    tokio::spawn(async move {
        loop {
            match scheduled_backup_due(&state).await {
                Ok(true) => {
                    let include_files = state.config.backup.include_files;
                    if let Err(error) = run_backup(&state, "scheduled", include_files).await {
                        tracing::error!("scheduled backup failed: {error:#}");
                    }
                }
                Ok(false) => {}
                Err(error) => tracing::error!("check scheduled backup due time failed: {error:#}"),
            }
            let check_minutes = state.config.backup.interval_minutes.min(5);
            tokio::time::sleep(Duration::from_secs(check_minutes.saturating_mul(60))).await;
        }
    });
}

pub async fn run_backup(
    state: &AppState,
    trigger_kind: &str,
    includes_files: bool,
) -> Result<BackupRun> {
    if !matches!(trigger_kind, "manual" | "scheduled") {
        bail!("unsupported backup trigger");
    }
    let id = Uuid::new_v4();
    let started_at = Utc::now();
    let timer = Instant::now();
    let _operation = state.lock_backup_operation().await;
    let result = create_archive(state, id, includes_files).await;
    let completed_at = Utc::now();
    let duration_ms = timer.elapsed().as_millis().min(i64::MAX as u128) as i64;
    let database_kind = state.database_backend();

    let row = match result {
        Ok(artifact) => {
            let row = insert_run(
                state,
                RunInsert {
                    id,
                    trigger_kind,
                    status: "succeeded",
                    database_kind,
                    includes_files,
                    artifact_path: Some(artifact.path.to_string_lossy().into_owned()),
                    artifact_sha256: Some(artifact.sha256),
                    artifact_size_bytes: Some(artifact.size_bytes),
                    manifest_created_at: Some(artifact.manifest.created_at),
                    started_at,
                    completed_at,
                    duration_ms,
                    error: None,
                },
            )
            .await?;
            enforce_retention(state).await?;
            row
        }
        Err(error) => {
            tracing::error!(backup_id = %id, "backup run failed: {error:#}");
            let row = insert_run(
                state,
                RunInsert {
                    id,
                    trigger_kind,
                    status: "failed",
                    database_kind,
                    includes_files,
                    artifact_path: None,
                    artifact_sha256: None,
                    artifact_size_bytes: None,
                    manifest_created_at: None,
                    started_at,
                    completed_at,
                    duration_ms,
                    error: Some("备份失败，请检查服务器日志"),
                },
            )
            .await?;
            return Err(error.context(format!("backup run {} failed", row.id)));
        }
    };
    Ok(row)
}

pub async fn export_state(
    state: &AppState,
    output: &Path,
    includes_files: bool,
) -> Result<BackupManifest> {
    if includes_files && state.attachment_store().oss_enabled() {
        bail!("object storage mode does not support attachment backup");
    }
    if !includes_files {
        return export_database(state, output, false).await;
    }

    let previous_lock = state.chat_rooms_locked().await?;
    let maintenance = state.begin_maintenance();
    let _requests = state.lock_requests_for_maintenance().await;
    state.set_chat_rooms_locked(true).await?;
    state.disconnect_all_chat_rooms(BACKUP_LOCK_REASON).await;
    let _write_barrier = match state.work_queue().maintenance().await {
        Ok(permits) => permits,
        Err(_) => {
            let _ = state.set_chat_rooms_locked(previous_lock).await;
            bail!("wait for in-flight writes before backup");
        }
    };
    let result = export_database(state, output, true).await;
    let unlock = state.set_chat_rooms_locked(previous_lock).await;
    drop(maintenance);
    unlock?;
    result
}

async fn export_database(
    state: &AppState,
    output: &Path,
    includes_files: bool,
) -> Result<BackupManifest> {
    match state.database_pool() {
        crate::storage::DatabasePool::Sqlite(pool) => {
            super::export_sqlite_scoped(&state.config, pool, output, includes_files).await
        }
        crate::storage::DatabasePool::Postgres(_) => {
            super::export_postgres_scoped(
                &state.config,
                &state.config.database.postgres_url,
                output,
                includes_files,
            )
            .await
        }
    }
}

struct Artifact {
    path: PathBuf,
    sha256: String,
    size_bytes: i64,
    manifest: BackupManifest,
}

async fn create_archive(state: &AppState, id: Uuid, includes_files: bool) -> Result<Artifact> {
    let destination = absolute_normalized(&state.config.backup.directory)?;
    let attachments = absolute_normalized(&state.config.attachments.directory)?;
    if destination.starts_with(&attachments) {
        bail!("backup directory must not be inside the attachment directory");
    }
    fs::create_dir_all(&destination)
        .with_context(|| format!("create backup destination {}", destination.display()))?;
    let work = super::create_work_directory(&state.config, "scheduled")?;
    let _cleanup = WorkCleanup(work.clone());
    let package = work.join("package");
    let manifest = export_state(state, &package, includes_files).await?;
    super::read_and_verify(&package).context("verify generated backup package")?;

    let scope = if includes_files { "complete" } else { "data" };
    let filename = format!(
        "chat-room-auto-{scope}-{}-{}.{}",
        manifest.created_at.format("%Y%m%d-%H%M%S-%3f"),
        id.simple(),
        super::ARCHIVE_EXTENSION
    );
    let artifact = destination.join(filename);
    let staged = sibling_temp_path(&artifact, "pack")?;
    let package_for_task = package.clone();
    let staged_for_task = staged.clone();
    tokio::task::spawn_blocking(move || super::pack_archive(&package_for_task, &staged_for_task))
        .await
        .context("join backup archive task")??;
    fs::rename(&staged, &artifact).context("publish backup archive")?;
    let record = file_record(&artifact, Path::new("archive.tar.gz"))?;
    Ok(Artifact {
        path: artifact,
        sha256: record.sha256,
        size_bytes: i64::try_from(record.size_bytes).unwrap_or(i64::MAX),
        manifest,
    })
}

async fn enforce_retention(state: &AppState) -> Result<()> {
    let destination = absolute_normalized(&state.config.backup.directory)?;
    let mut artifacts = fs::read_dir(&destination)?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| {
                    name.starts_with("chat-room-auto-") && name.ends_with(".tar.gz")
                })
        })
        .collect::<Vec<_>>();
    artifacts.sort();
    let remove_count = artifacts
        .len()
        .saturating_sub(state.config.backup.retention_count);
    for path in artifacts.into_iter().take(remove_count) {
        fs::remove_file(&path)
            .with_context(|| format!("remove expired backup {}", path.display()))?;
        let path = path.to_string_lossy().into_owned();
        with_pool!(state, |pool| {
            sqlx::query("UPDATE backup_runs SET artifact_path = NULL WHERE artifact_path = $1")
                .bind(&path)
                .execute(pool)
                .await
                .map(|_| ())
        })?;
    }
    Ok(())
}
