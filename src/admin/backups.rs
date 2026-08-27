//! Administrator-only browser backup export and restore endpoints.

use std::path::Path;

use axum::{
    extract::{Multipart, Request, State},
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json, Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;
use utoipa::ToSchema;

use super::{access::require_admin, backup_transfer::WorkDirectory, indexes};
use crate::{backup, state::SharedState};

const RESTORE_REASON: &str = "database restore in progress";

pub fn routes() -> Router<SharedState> {
    Router::new()
        .route("/api/admin/backups/export", axum::routing::post(export))
        .route(
            "/api/admin/backups/restore",
            axum::routing::post(restore).layer(axum::extract::DefaultBodyLimit::disable()),
        )
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ExportBackupRequest {
    #[serde(default)]
    include_files: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct RestoreBackupResult {
    backup_created_at: DateTime<Utc>,
    included_files: bool,
    previous_files_preserved: bool,
    redis_keys_cleared: usize,
    vector_messages_queued: u64,
    chat_rooms_locked: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct BackupApiError {
    error: String,
}

pub async fn reject_during_restore(
    State(state): State<SharedState>,
    request: Request,
    next: Next,
) -> Response {
    let path = request.uri().path();
    if path == "/api/config" {
        return next.run(request).await;
    }
    if state.maintenance_active() {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    }
    if matches!(
        path,
        "/api/admin/backups/export" | "/api/admin/backups/restore"
    ) {
        return next.run(request).await;
    }
    let _request_guard = state.lock_request().await;
    if state.maintenance_active() {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    }
    next.run(request).await
}

#[utoipa::path(
    post,
    path = "/api/admin/backups/export",
    request_body = ExportBackupRequest,
    responses(
        (status = 200, description = "Verified PostgreSQL backup archive"),
        (status = 401, description = "Missing or expired session"),
        (status = 403, description = "Account is not a system administrator"),
        (status = 409, description = "Backup mode is unavailable for this storage backend")
    )
)]
pub async fn export(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(request): Json<ExportBackupRequest>,
) -> Response {
    if let Err(status) = require_admin(&state, &headers).await {
        return status.into_response();
    }
    if state.database_backend() != "postgres" {
        return api_error(StatusCode::CONFLICT, "当前仅支持 PostgreSQL 数据库备份");
    }
    if request.include_files && state.attachment_store().oss_enabled() {
        return api_error(StatusCode::CONFLICT, "对象存储模式暂不支持文件备份");
    }

    let _operation = state.lock_backup_operation().await;
    let work = match WorkDirectory::create(&state.config, "export") {
        Ok(work) => work,
        Err(error) => return internal_error("创建备份工作目录", error),
    };
    let package = work.path.join("package");
    let archive = work.path.join("chat-room-backup.tar.gz");
    let previous_lock = match state.chat_rooms_locked().await {
        Ok(value) => value,
        Err(error) => return internal_error("读取系统锁", error.into()),
    };

    let manifest = if request.include_files {
        let maintenance = state.begin_maintenance();
        let _requests = state.lock_requests_for_maintenance().await;
        if let Err(error) = state.set_chat_rooms_locked(true).await {
            return internal_error("锁定聊天室", error.into());
        }
        state.disconnect_all_chat_rooms(RESTORE_REASON).await;
        let _write_barrier = match state.work_queue().maintenance().await {
            Ok(permits) => permits,
            Err(_) => {
                let _ = state.set_chat_rooms_locked(previous_lock).await;
                drop(maintenance);
                return internal_error("等待在途写入", anyhow::anyhow!("work queue timed out"));
            }
        };
        let result = backup::export_postgres_scoped(
            &state.config,
            &state.config.database.postgres_url,
            &package,
            true,
        )
        .await;
        let unlock_result = state.set_chat_rooms_locked(previous_lock).await;
        drop(maintenance);
        if let Err(error) = unlock_result {
            return internal_error("恢复系统锁状态", error.into());
        }
        match result {
            Ok(manifest) => manifest,
            Err(error) => return internal_error("导出备份", error),
        }
    } else {
        match backup::export_postgres_scoped(
            &state.config,
            &state.config.database.postgres_url,
            &package,
            false,
        )
        .await
        {
            Ok(manifest) => manifest,
            Err(error) => return internal_error("导出备份", error),
        }
    };

    let package_for_archive = package.clone();
    let archive_for_task = archive.clone();
    if let Err(error) = tokio::task::spawn_blocking(move || {
        backup::pack_archive(&package_for_archive, &archive_for_task)
    })
    .await
    .map_err(anyhow::Error::from)
    .and_then(|result| result)
    {
        return internal_error("压缩备份", error);
    }
    let scope = if manifest.includes_files {
        "complete"
    } else {
        "data"
    };
    let filename = format!(
        "chat-room-{scope}-{}.{}",
        manifest.created_at.format("%Y%m%d-%H%M%S"),
        backup::ARCHIVE_EXTENSION
    );
    match work.download(&archive, &filename).await {
        Ok(response) => response,
        Err(error) => internal_error("打开备份归档", error),
    }
}

#[utoipa::path(
    post,
    path = "/api/admin/backups/restore",
    request_body(content = String, content_type = "multipart/form-data"),
    responses(
        (status = 200, description = "Backup restored; chat remains locked", body = RestoreBackupResult),
        (status = 400, description = "No backup archive was uploaded", body = BackupApiError),
        (status = 401, description = "Missing or expired session"),
        (status = 403, description = "Account is not a system administrator"),
        (status = 422, description = "Archive or checksum validation failed", body = BackupApiError)
    )
)]
pub async fn restore(
    State(state): State<SharedState>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Response {
    if let Err(status) = require_admin(&state, &headers).await {
        return status.into_response();
    }
    if state.database_backend() != "postgres" {
        return api_error(StatusCode::CONFLICT, "当前仅支持 PostgreSQL 数据库恢复");
    }

    let _operation = state.lock_backup_operation().await;
    let work = match WorkDirectory::create(&state.config, "restore") {
        Ok(work) => work,
        Err(error) => return internal_error("创建恢复工作目录", error),
    };
    let archive = work.path.join("upload.tar.gz");
    if let Err(response) = receive_archive(&mut multipart, &archive).await {
        return response;
    }
    let package = work.path.join("package");
    let archive_for_task = archive.clone();
    let package_for_task = package.clone();
    let unpacked = tokio::task::spawn_blocking(move || {
        backup::unpack_archive(&archive_for_task, &package_for_task)
    })
    .await
    .map_err(anyhow::Error::from)
    .and_then(|result| result);
    if let Err(error) = unpacked {
        tracing::warn!("reject invalid backup archive: {error:#}");
        return api_error(StatusCode::UNPROCESSABLE_ENTITY, "备份归档无效或已损坏");
    }
    let manifest = match backup::read_and_verify(&package) {
        Ok(manifest) => manifest,
        Err(error) => {
            tracing::warn!("reject invalid backup package: {error:#}");
            return api_error(StatusCode::UNPROCESSABLE_ENTITY, "备份清单或文件校验失败");
        }
    };
    if manifest.includes_files && state.attachment_store().oss_enabled() {
        return api_error(StatusCode::CONFLICT, "对象存储模式暂不支持文件恢复");
    }

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
    let database_connections = match state.lock_postgres_connections().await {
        Ok(connections) => connections,
        Err(error) => return internal_error("等待数据库操作", error),
    };
    let restore_result =
        backup::restore_postgres(&state.config, &state.config.database.postgres_url, &package)
            .await;
    let connection_result = database_connections.close().await;
    if let Err(error) = connection_result {
        if let Err(lock_error) = state.set_chat_rooms_locked(true).await {
            tracing::error!("keep chat locked after connection refresh failed: {lock_error}");
        }
        return internal_error("刷新数据库连接", error);
    }
    let outcome = match restore_result {
        Ok(outcome) => outcome,
        Err(error) => {
            if let Err(lock_error) = state.set_chat_rooms_locked(true).await {
                tracing::error!("keep chat locked after failed restore: {lock_error}");
            }
            return internal_error("恢复备份", error);
        }
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

    Json(RestoreBackupResult {
        backup_created_at: manifest.created_at,
        included_files: outcome.includes_files,
        previous_files_preserved: outcome.previous_attachments.is_some(),
        redis_keys_cleared: outcome.redis_keys_cleared,
        vector_messages_queued: index_sync.vector_messages,
        chat_rooms_locked: true,
    })
    .into_response()
}

async fn receive_archive(multipart: &mut Multipart, output: &Path) -> Result<(), Response> {
    while let Some(mut field) = multipart.next_field().await.map_err(|error| {
        tracing::warn!("read backup multipart failed: {error}");
        api_error(StatusCode::BAD_REQUEST, "无法读取上传的备份文件")
    })? {
        if field.name() != Some("file") {
            continue;
        }
        let mut file = tokio::fs::File::create(output)
            .await
            .map_err(|error| internal_error("创建备份上传文件", error.into()))?;
        let mut size = 0_u64;
        while let Some(chunk) = field.chunk().await.map_err(|error| {
            tracing::warn!("read backup upload failed: {error}");
            api_error(StatusCode::BAD_REQUEST, "备份文件上传失败")
        })? {
            size = size.saturating_add(chunk.len() as u64);
            file.write_all(&chunk)
                .await
                .map_err(|error| internal_error("写入备份上传文件", error.into()))?;
        }
        file.flush()
            .await
            .map_err(|error| internal_error("写入备份上传文件", error.into()))?;
        return if size == 0 {
            Err(api_error(StatusCode::BAD_REQUEST, "备份文件不能为空"))
        } else {
            Ok(())
        };
    }
    Err(api_error(StatusCode::BAD_REQUEST, "请选择备份文件"))
}

fn api_error(status: StatusCode, message: &str) -> Response {
    (
        status,
        Json(BackupApiError {
            error: message.into(),
        }),
    )
        .into_response()
}

fn internal_error(operation: &str, error: anyhow::Error) -> Response {
    tracing::error!("{operation} failed: {error:#}");
    api_error(
        StatusCode::INTERNAL_SERVER_ERROR,
        &format!("{operation}失败，请检查服务器日志"),
    )
}
