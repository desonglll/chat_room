//! Phase one of sending an album: publish every staged upload under its content address
//! BEFORE any database row exists. Publishing is idempotent and never deletes an object,
//! so a failure here or in the later transaction leaves no message behind — at worst an
//! unreferenced content-addressed object, exactly like a failed single-file completion.
//!
//! Retry safety: publishing moves the staging file away. The verified digest is written to
//! the session row first, so a retry of the same upload (same session, staging gone,
//! `received_bytes == declared`) can reuse the published object instead of failing.

use anyhow::{bail, Context, Result};
use chrono::Utc;
use uuid::Uuid;

use crate::attachment_upload_sessions::AttachmentUploadSession;
use crate::state::{with_pool, AppState};

/// One upload whose bytes are durably published and ready to be referenced.
pub(super) struct PromotedItem {
    pub upload_id: Uuid,
    pub file_name: String,
    pub mime_type: String,
    pub size_bytes: i64,
    pub content_hash: String,
    pub storage_key: String,
}

impl AppState {
    pub(super) async fn promote_album_upload(
        &self,
        session: &AttachmentUploadSession,
    ) -> Result<PromotedItem> {
        let upload_id = session.id;
        let declared = u64::try_from(session.declared_size_bytes).context("negative size")?;
        let staged = self
            .attachment_store()
            .chunked_upload_size(upload_id)
            .await?;
        let (content_hash, storage_key) = if staged == declared {
            self.publish_staged(session, declared).await?
        } else if staged == 0 {
            self.reuse_published(session).await?
        } else {
            bail!("upload {upload_id} staging holds {staged} of {declared} bytes");
        };
        Ok(PromotedItem {
            upload_id,
            file_name: session.file_name.clone(),
            mime_type: session.mime_type.clone(),
            size_bytes: session.declared_size_bytes,
            content_hash,
            storage_key,
        })
    }

    async fn publish_staged(
        &self,
        session: &AttachmentUploadSession,
        declared: u64,
    ) -> Result<(String, String)> {
        let upload_id = session.id;
        let content_hash = match self
            .upload_hashes()
            .completed_digest(upload_id, declared)
            .await
        {
            Some(hash) => hash,
            None => self.attachment_store().hash_chunked(upload_id).await?,
        };
        if session
            .content_hash
            .as_deref()
            .is_some_and(|expected| expected != content_hash)
        {
            bail!("uploaded content does not match its declared SHA-256");
        }
        // Record the verified digest before the staging file moves (see module docs).
        with_pool!(self, |pool| {
            sqlx::query(
                "UPDATE attachment_uploads SET content_hash = $1, updated_at = $2 WHERE id = $3",
            )
            .bind(&content_hash)
            .bind(Utc::now())
            .bind(upload_id)
            .execute(pool)
            .await
            .map(|_| ())
        })?;
        let _guard = self.content_hash_locks().lock(&content_hash).await;
        let storage_key = match self.healthy_storage_key(&content_hash).await? {
            Some(key) => {
                self.attachment_store().discard_chunked(upload_id).await?;
                key
            }
            None => {
                self.attachment_store()
                    .commit_chunked(upload_id, &content_hash)
                    .await?;
                content_hash.clone()
            }
        };
        Ok((content_hash, storage_key))
    }

    /// Staging is gone but every byte was received: either an earlier album attempt published
    /// it (digest recorded above, verified) or the create step deduplicated it against the
    /// caller's own attachment. Both leave a digest on the session; the object must exist.
    async fn reuse_published(&self, session: &AttachmentUploadSession) -> Result<(String, String)> {
        let Some(content_hash) = session.content_hash.clone() else {
            bail!("upload {} has no staged bytes", session.id);
        };
        let owned = self
            .healthy_owned_storage_key(
                &content_hash,
                session.uploader_id,
                session.declared_size_bytes,
            )
            .await?;
        let storage_key = match owned {
            Some(key) => key,
            None if self.attachment_store().exists(&content_hash).await? => content_hash.clone(),
            None => bail!("upload {} lost its published object", session.id),
        };
        Ok((content_hash, storage_key))
    }
}
