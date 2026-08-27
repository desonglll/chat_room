use std::path::Path;

use axum::{
    extract::Multipart,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use tokio::io::AsyncWriteExt;
use utoipa::ToSchema;

#[derive(Debug, Serialize, ToSchema)]
pub struct BackupApiError {
    error: String,
}

pub(super) async fn receive_archive(
    multipart: &mut Multipart,
    output: &Path,
) -> Result<(), Response> {
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

pub(super) fn api_error(status: StatusCode, message: &str) -> Response {
    (
        status,
        Json(BackupApiError {
            error: message.into(),
        }),
    )
        .into_response()
}

pub(super) fn internal_error(operation: &str, error: anyhow::Error) -> Response {
    tracing::error!("{operation} failed: {error:#}");
    api_error(
        StatusCode::INTERNAL_SERVER_ERROR,
        &format!("{operation}失败，请检查服务器日志"),
    )
}
