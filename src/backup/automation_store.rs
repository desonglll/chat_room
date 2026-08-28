use std::path::Path;

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::FromRow;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::state::{with_pool, AppState};

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct BackupRun {
    pub id: Uuid,
    #[serde(rename = "trigger")]
    pub trigger_kind: String,
    pub status: String,
    pub database_kind: String,
    pub target_backend: String,
    pub includes_files: bool,
    pub artifact_path: Option<String>,
    pub artifact_sha256: Option<String>,
    pub artifact_size_bytes: Option<i64>,
    pub manifest_created_at: Option<DateTime<Utc>>,
    pub started_at: DateTime<Utc>,
    pub completed_at: DateTime<Utc>,
    pub duration_ms: i64,
    pub error: Option<String>,
    pub checksum_status: String,
    pub artifact_available: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct BackupStatus {
    pub enabled: bool,
    pub interval_minutes: u64,
    pub retention_count: usize,
    pub target_backend: String,
    pub include_files: bool,
    pub rpo_minutes: u64,
    pub runs: Vec<BackupRun>,
}

#[derive(FromRow)]
struct BackupRunRow {
    id: Uuid,
    trigger_kind: String,
    status: String,
    database_kind: String,
    target_backend: String,
    includes_files: bool,
    artifact_path: Option<String>,
    artifact_sha256: Option<String>,
    artifact_size_bytes: Option<i64>,
    manifest_created_at: Option<DateTime<Utc>>,
    started_at: DateTime<Utc>,
    completed_at: Option<DateTime<Utc>>,
    duration_ms: Option<i64>,
    error: Option<String>,
}

impl BackupRunRow {
    fn view(self) -> BackupRun {
        let artifact_available = self
            .artifact_path
            .as_deref()
            .is_some_and(|path| Path::new(path).is_file());
        BackupRun {
            id: self.id,
            trigger_kind: self.trigger_kind,
            status: self.status,
            database_kind: self.database_kind,
            target_backend: self.target_backend,
            includes_files: self.includes_files,
            artifact_path: self.artifact_path,
            checksum_status: if self.artifact_sha256.is_some() {
                "verified".into()
            } else {
                "unavailable".into()
            },
            artifact_sha256: self.artifact_sha256,
            artifact_size_bytes: self.artifact_size_bytes,
            manifest_created_at: self.manifest_created_at,
            started_at: self.started_at,
            completed_at: self.completed_at.unwrap_or(self.started_at),
            duration_ms: self.duration_ms.unwrap_or_default(),
            error: self.error,
            artifact_available,
        }
    }
}

pub async fn status(state: &AppState) -> Result<BackupStatus> {
    let rows: Vec<BackupRunRow> = with_pool!(state, |pool| {
        sqlx::query_as(
            "SELECT id, trigger_kind, status, database_kind, target_backend, includes_files, \
             artifact_path, artifact_sha256, artifact_size_bytes, manifest_created_at, \
             started_at, completed_at, duration_ms, error FROM backup_runs \
             ORDER BY started_at DESC, id DESC LIMIT 20",
        )
        .fetch_all(pool)
        .await
    })?;
    Ok(BackupStatus {
        enabled: state.config.backup.enabled,
        interval_minutes: state.config.backup.interval_minutes,
        retention_count: state.config.backup.retention_count,
        target_backend: state.config.backup.target_backend.clone(),
        include_files: state.config.backup.include_files,
        rpo_minutes: state.config.backup.interval_minutes,
        runs: rows.into_iter().map(BackupRunRow::view).collect(),
    })
}

pub(super) struct RunInsert<'a> {
    pub id: Uuid,
    pub trigger_kind: &'a str,
    pub status: &'a str,
    pub database_kind: &'a str,
    pub includes_files: bool,
    pub artifact_path: Option<String>,
    pub artifact_sha256: Option<String>,
    pub artifact_size_bytes: Option<i64>,
    pub manifest_created_at: Option<DateTime<Utc>>,
    pub started_at: DateTime<Utc>,
    pub completed_at: DateTime<Utc>,
    pub duration_ms: i64,
    pub error: Option<&'a str>,
}

pub(super) async fn insert_run(state: &AppState, run: RunInsert<'_>) -> Result<BackupRun> {
    let row: BackupRunRow = with_pool!(state, |pool| {
        sqlx::query_as(
            "INSERT INTO backup_runs \
             (id, trigger_kind, status, database_kind, target_backend, includes_files, \
              artifact_path, artifact_sha256, artifact_size_bytes, manifest_created_at, \
              started_at, completed_at, duration_ms, error) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14) \
             RETURNING id, trigger_kind, status, database_kind, target_backend, includes_files, \
              artifact_path, artifact_sha256, artifact_size_bytes, manifest_created_at, \
              started_at, completed_at, duration_ms, error",
        )
        .bind(run.id)
        .bind(run.trigger_kind)
        .bind(run.status)
        .bind(run.database_kind)
        .bind(&state.config.backup.target_backend)
        .bind(run.includes_files)
        .bind(run.artifact_path)
        .bind(run.artifact_sha256)
        .bind(run.artifact_size_bytes)
        .bind(run.manifest_created_at)
        .bind(run.started_at)
        .bind(run.completed_at)
        .bind(run.duration_ms)
        .bind(run.error)
        .fetch_one(pool)
        .await
    })?;
    Ok(row.view())
}

pub(super) async fn scheduled_backup_due(state: &AppState) -> Result<bool> {
    let latest: Option<DateTime<Utc>> = with_pool!(state, |pool| {
        sqlx::query_scalar(
            "SELECT MAX(started_at) FROM backup_runs WHERE trigger_kind = 'scheduled'",
        )
        .fetch_one(pool)
        .await
    })?;
    let interval = chrono::Duration::minutes(
        i64::try_from(state.config.backup.interval_minutes).unwrap_or(i64::MAX),
    );
    Ok(latest.is_none_or(|started| Utc::now().signed_duration_since(started) >= interval))
}
