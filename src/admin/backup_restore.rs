use std::time::Instant;

use axum::{
    extract::{Multipart, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;

use super::{
    access::require_admin,
    backup_http::{api_error, internal_error, receive_archive},
    backup_transfer::WorkDirectory,
    indexes,
};
use crate::{audit::AuditEventDraft, backup, state::SharedState};

const RESTORE_REASON: &str = "database restore in progress";
const RESTORE_CONFIRMATION: &str = "RESTORE";

#[derive(Debug, Serialize, ToSchema)]
pub struct RestoreValidationResult {
    valid: bool,
    backup_created_at: DateTime<Utc>,
    database_kind: String,
    included_files: bool,
    file_count: usize,
    total_bytes: u64,
    checksum_status: &'static str,
    validation_duration_ms: u64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct RestoreBackupResult {
    backup_created_at: DateTime<Utc>,
    included_files: bool,
    previous_database_preserved: bool,
    previous_files_preserved: bool,
    redis_keys_cleared: usize,
    vector_messages_queued: u64,
    chat_rooms_locked: bool,
    restore_duration_ms: u64,
}

pub(super) async fn validate(
    State(state): State<SharedState>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Response {
    let actor = match require_admin(&state, &headers).await {
        Ok(actor) => actor,
        Err(status) => return status.into_response(),
    };
    let work = match WorkDirectory::create(&state.config, "validate") {
        Ok(work) => work,
        Err(error) => return internal_error("创建校验工作目录", error),
    };
    let archive = work.path.join("upload.tar.gz");
    if let Err(response) = receive_archive(&mut multipart, &archive).await {
        return response;
    }
    let started = Instant::now();
    let package = work.path.join("package");
    if let Some(response) = unpack_and_verify_archive(&archive, &package).await {
        return response;
    }
    let manifest = match backup::read_and_verify(&package) {
        Ok(manifest) => manifest,
        Err(error) => {
            tracing::warn!("reject invalid backup package: {error:#}");
            return api_error(StatusCode::UNPROCESSABLE_ENTITY, "备份清单或文件校验失败");
        }
    };
    if let Some(response) = validate_restore_scope(&state, &manifest) {
        return response;
    }
    if let Err(error) = state
        .record_audit_event(
            AuditEventDraft::system(&actor, "backup.restore_validated")
                .target_type("backup")
                .detail("database_kind", &manifest.database_kind)
                .detail("included_files", manifest.includes_files),
        )
        .await
    {
        return internal_error("记录备份校验审计", error.into());
    }
    Json(RestoreValidationResult {
        valid: true,
        backup_created_at: manifest.created_at,
        database_kind: manifest.database_kind,
        included_files: manifest.includes_files,
        file_count: manifest.files.len(),
        total_bytes: manifest.files.iter().map(|file| file.size_bytes).sum(),
        checksum_status: "verified",
        validation_duration_ms: started.elapsed().as_millis().min(u64::MAX as u128) as u64,
    })
    .into_response()
}

pub(super) async fn execute(
    State(state): State<SharedState>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Response {
    let actor = match require_admin(&state, &headers).await {
        Ok(actor) => actor,
        Err(status) => return status.into_response(),
    };
    let _operation = state.lock_backup_operation().await;
    let work = match WorkDirectory::create(&state.config, "restore") {
        Ok(work) => work,
        Err(error) => return internal_error("创建恢复工作目录", error),
    };
    let archive = work.path.join("upload.tar.gz");
    let confirmation = match receive_archive(&mut multipart, &archive).await {
        Ok(confirmation) => confirmation,
        Err(response) => return response,
    };
    if confirmation.as_deref() != Some(RESTORE_CONFIRMATION) {
        return api_error(StatusCode::BAD_REQUEST, "执行恢复需要二次确认");
    }
    let package = work.path.join("package");
    if let Some(response) = unpack_and_verify_archive(&archive, &package).await {
        return response;
    }
    let manifest = match backup::read_and_verify(&package) {
        Ok(manifest) => manifest,
        Err(_) => return api_error(StatusCode::UNPROCESSABLE_ENTITY, "备份清单或文件校验失败"),
    };
    if let Some(response) = validate_restore_scope(&state, &manifest) {
        return response;
    }
    if let Err(error) = state
        .record_audit_event(
            AuditEventDraft::system(&actor, "backup.restore_requested")
                .target_type("backup")
                .detail("database_kind", &manifest.database_kind)
                .detail("included_files", manifest.includes_files),
        )
        .await
    {
        return internal_error("记录备份恢复审计", error.into());
    }

    let restore_started = Instant::now();
    let maintenance = state.begin_maintenance();
    let _requests = state.lock_requests_for_maintenance().await;
    if let Err(error) = state.set_chat_rooms_locked(true).await {
        return internal_error("锁定聊天室", error.into());
    }
    state.disconnect_all_chat_rooms(RESTORE_REASON).await;
    let _write_barrier = match state.work_queue().maintenance().await {
        Ok(permits) => permits,
        Err(_) => return internal_error("等待在途写入", anyhow::anyhow!("work queue timed out")),
    };
    let restore_result = if state.database_backend() == "postgres" {
        let connections = match state.lock_postgres_connections().await {
            Ok(connections) => connections,
            Err(error) => return internal_error("等待数据库操作", error),
        };
        let result =
            backup::restore_postgres(&state.config, &state.config.database.postgres_url, &package)
                .await;
        if let Err(error) = connections.close().await {
            return internal_error("刷新数据库连接", error);
        }
        result
    } else {
        let connections = match state.lock_sqlite_connections().await {
            Ok(connections) => connections,
            Err(error) => return internal_error("等待数据库操作", error),
        };
        let result = backup::restore_sqlite(&state.config, &package).await;
        if let Err(error) = connections.close().await {
            return internal_error("刷新数据库连接", error);
        }
        result
    };
    let outcome = match restore_result {
        Ok(outcome) => outcome,
        Err(error) => return internal_error("恢复备份", error),
    };
    if let Err(error) = state.set_chat_rooms_locked(true).await {
        return internal_error("恢复系统锁", error.into());
    }
    if let Err(error) = state.reload_room_cache().await {
        return internal_error("刷新服务状态", error);
    }
    let index_sync = match indexes::sync_enabled(&state).await {
        Ok(result) => result,
        Err(error) => return internal_error("重新同步派生索引", error.into()),
    };
    drop(maintenance);
    if let Err(error) = state
        .record_audit_event(
            AuditEventDraft::system(&actor, "backup.restore_completed")
                .target_type("backup")
                .detail("included_files", outcome.includes_files),
        )
        .await
    {
        tracing::error!("record completed backup restore audit failed: {error}");
    }
    Json(RestoreBackupResult {
        backup_created_at: manifest.created_at,
        included_files: outcome.includes_files,
        previous_database_preserved: outcome.previous_database.is_some(),
        previous_files_preserved: outcome.previous_attachments.is_some(),
        redis_keys_cleared: outcome.redis_keys_cleared,
        vector_messages_queued: index_sync.vector_messages,
        chat_rooms_locked: true,
        restore_duration_ms: restore_started.elapsed().as_millis().min(u64::MAX as u128) as u64,
    })
    .into_response()
}

async fn unpack_and_verify_archive(
    archive: &std::path::Path,
    package: &std::path::Path,
) -> Option<Response> {
    let archive = archive.to_path_buf();
    let package = package.to_path_buf();
    let result = tokio::task::spawn_blocking(move || backup::unpack_archive(&archive, &package))
        .await
        .map_err(anyhow::Error::from)
        .and_then(|result| result);
    result.err().map(|error| {
        tracing::warn!("reject invalid backup archive: {error:#}");
        api_error(StatusCode::UNPROCESSABLE_ENTITY, "备份归档无效或已损坏")
    })
}

fn validate_restore_scope(
    state: &crate::state::AppState,
    manifest: &backup::BackupManifest,
) -> Option<Response> {
    if manifest.database_kind != state.database_backend() {
        return Some(api_error(
            StatusCode::CONFLICT,
            "备份数据库类型与当前服务不一致",
        ));
    }
    if manifest.includes_files && state.attachment_store().oss_enabled() {
        return Some(api_error(
            StatusCode::CONFLICT,
            "对象存储模式暂不支持文件恢复",
        ));
    }
    None
}
