//! HTTP payloads for resumable and direct attachment uploads.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use super::storage::DirectUploadTarget;

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateUploadRequest {
    pub file_name: String,
    pub mime_type: String,
    pub size_bytes: i64,
    #[serde(default)]
    pub fingerprint: String,
    #[serde(default)]
    pub content_hash: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CreateUploadResponse {
    pub upload_id: Uuid,
    pub received_bytes: i64,
    pub declared_size_bytes: i64,
    pub deduplicated: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direct_upload: Option<DirectUploadTarget>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ChunkResponse {
    pub received_bytes: i64,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CompleteUploadRequest {
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub reply_to: Option<Uuid>,
    #[serde(default)]
    pub is_sensitive: bool,
}
