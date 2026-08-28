use axum::{extract::State, http::HeaderMap, response::IntoResponse, Json};
use chrono::Datelike;

use super::{
    access::require_admin,
    backup_http::{api_error, internal_error},
    backup_transfer::WorkDirectory,
    backups::ExportBackupRequest,
};
use crate::{audit::AuditEventDraft, backup, state::SharedState};

pub(super) async fn export(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(request): Json<ExportBackupRequest>,
) -> axum::response::Response {
    let actor = match require_admin(&state, &headers).await {
        Ok(actor) => actor,
        Err(status) => return status.into_response(),
    };
    if request.include_files && state.attachment_store().oss_enabled() {
        return api_error(
            axum::http::StatusCode::CONFLICT,
            "对象存储模式暂不支持文件备份",
        );
    }
    if let Err(error) = state
        .record_audit_event(
            AuditEventDraft::system(&actor, "backup.export_requested")
                .target_type("backup")
                .detail("include_files", request.include_files),
        )
        .await
    {
        return internal_error("记录备份导出审计", error.into());
    }

    let _operation = state.lock_backup_operation().await;
    let work = match WorkDirectory::create(&state.config, "export") {
        Ok(work) => work,
        Err(error) => return internal_error("创建备份工作目录", error),
    };
    let package = work.path.join("package");
    let archive = work.path.join("chat-room-backup.tar.gz");
    let manifest = match backup::export_state(&state, &package, request.include_files).await {
        Ok(manifest) => manifest,
        Err(error) => return internal_error("导出备份", error),
    };
    if let Err(error) = backup::read_and_verify(&package) {
        return internal_error("校验备份", error);
    }
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
        "chat-room-{scope}-{:04}{:02}{:02}-{}.{}",
        manifest.created_at.year(),
        manifest.created_at.month(),
        manifest.created_at.day(),
        manifest.created_at.format("%H%M%S"),
        backup::ARCHIVE_EXTENSION
    );
    match work.download(&archive, &filename).await {
        Ok(response) => response,
        Err(error) => internal_error("打开备份归档", error),
    }
}
