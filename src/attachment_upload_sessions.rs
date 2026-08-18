//! Durable bookkeeping for resumable/chunked attachment uploads.

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::FromRow;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::state::{with_pool, AppState};

#[derive(Debug, Clone, Serialize, ToSchema, FromRow)]
pub struct AttachmentUploadSession {
    pub id: Uuid,
    pub room_id: Uuid,
    pub uploader_id: Uuid,
    pub file_name: String,
    pub mime_type: String,
    pub declared_size_bytes: i64,
    pub received_bytes: i64,
    pub fingerprint: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl AppState {
    /// Create a new chunked upload session, or hand back an existing in-progress
    /// one for the same (uploader, room, fingerprint) — the resume handshake: the
    /// client re-selects the same file and this lets it continue instead of
    /// restarting from byte zero.
    pub async fn create_or_resume_attachment_upload(
        &self,
        room_id: Uuid,
        uploader_id: Uuid,
        file_name: &str,
        mime_type: &str,
        declared_size_bytes: i64,
        fingerprint: &str,
    ) -> Result<AttachmentUploadSession, sqlx::Error> {
        if !fingerprint.is_empty() {
            let existing: Option<AttachmentUploadSession> = with_pool!(self, |pool| {
                sqlx::query_as(
                    "SELECT id, room_id, uploader_id, file_name, mime_type, declared_size_bytes, \
                     received_bytes, fingerprint, status, created_at, updated_at \
                     FROM attachment_uploads \
                     WHERE room_id = $1 AND uploader_id = $2 AND fingerprint = $3 \
                     AND status = 'in_progress'",
                )
                .bind(room_id)
                .bind(uploader_id)
                .bind(fingerprint)
                .fetch_optional(pool)
                .await
            })?;
            if let Some(session) = existing {
                return Ok(session);
            }
        }

        let id = Uuid::new_v4();
        let now = Utc::now();
        with_pool!(self, |pool| {
            sqlx::query(
                "INSERT INTO attachment_uploads \
                 (id, room_id, uploader_id, file_name, mime_type, declared_size_bytes, \
                  received_bytes, fingerprint, status, created_at, updated_at) \
                 VALUES ($1, $2, $3, $4, $5, $6, 0, $7, 'in_progress', $8, $9)",
            )
            .bind(id)
            .bind(room_id)
            .bind(uploader_id)
            .bind(file_name)
            .bind(mime_type)
            .bind(declared_size_bytes)
            .bind(fingerprint)
            .bind(now)
            .bind(now)
            .execute(pool)
            .await
            .map(|_| ())
        })?;
        Ok(AttachmentUploadSession {
            id,
            room_id,
            uploader_id,
            file_name: file_name.to_string(),
            mime_type: mime_type.to_string(),
            declared_size_bytes,
            received_bytes: 0,
            fingerprint: fingerprint.to_string(),
            status: "in_progress".to_string(),
            created_at: now,
            updated_at: now,
        })
    }

    pub async fn attachment_upload(
        &self,
        upload_id: Uuid,
    ) -> Result<Option<AttachmentUploadSession>, sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query_as(
                "SELECT id, room_id, uploader_id, file_name, mime_type, declared_size_bytes, \
                 received_bytes, fingerprint, status, created_at, updated_at \
                 FROM attachment_uploads WHERE id = $1",
            )
            .bind(upload_id)
            .fetch_optional(pool)
            .await
        })
    }

    pub async fn list_attachment_uploads(
        &self,
        room_id: Uuid,
        uploader_id: Uuid,
    ) -> Result<Vec<AttachmentUploadSession>, sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query_as(
                "SELECT id, room_id, uploader_id, file_name, mime_type, declared_size_bytes, \
                 received_bytes, fingerprint, status, created_at, updated_at \
                 FROM attachment_uploads \
                 WHERE room_id = $1 AND uploader_id = $2 AND status = 'in_progress' \
                 ORDER BY updated_at DESC",
            )
            .bind(room_id)
            .bind(uploader_id)
            .fetch_all(pool)
            .await
        })
    }

    pub async fn update_attachment_upload_progress(
        &self,
        upload_id: Uuid,
        received_bytes: i64,
    ) -> Result<(), sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query(
                "UPDATE attachment_uploads SET received_bytes = $1, updated_at = $2 \
                 WHERE id = $3",
            )
            .bind(received_bytes)
            .bind(Utc::now())
            .bind(upload_id)
            .execute(pool)
            .await
            .map(|_| ())
        })
    }

    pub async fn finish_attachment_upload(
        &self,
        upload_id: Uuid,
        status: &str,
    ) -> Result<(), sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query("UPDATE attachment_uploads SET status = $1, updated_at = $2 WHERE id = $3")
                .bind(status)
                .bind(Utc::now())
                .bind(upload_id)
                .execute(pool)
                .await
                .map(|_| ())
        })
    }

    pub async fn delete_attachment_upload(
        &self,
        upload_id: Uuid,
        uploader_id: Uuid,
    ) -> Result<bool, sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query("DELETE FROM attachment_uploads WHERE id = $1 AND uploader_id = $2")
                .bind(upload_id)
                .bind(uploader_id)
                .execute(pool)
                .await
                .map(|result| result.rows_affected() > 0)
        })
    }
}
