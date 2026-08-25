//! Persistence boundary for a browser-uploaded OSS object.

use anyhow::Result;
use uuid::Uuid;

use crate::models::{StoredMessage, User};
use crate::state::AppState;

impl AppState {
    #[allow(clippy::too_many_arguments)]
    pub async fn store_direct_attachment_message(
        &self,
        upload_id: Uuid,
        room_id: Uuid,
        sender: &User,
        sender_display_name: &str,
        file_name: String,
        mime_type: String,
        size_bytes: i64,
        is_sensitive: bool,
        content: &str,
        reply_to: Option<Uuid>,
        content_hash: String,
    ) -> Result<StoredMessage> {
        let direct_key = self
            .attachment_store()
            .commit_direct(upload_id, &content_hash, size_bytes)
            .await?;
        let _guard = self.content_hash_locks().lock(&content_hash).await;
        let existing_key = self.healthy_storage_key(&content_hash).await?;
        let (storage_key, owns_direct_object) = match existing_key {
            Some(existing) if existing != direct_key => {
                self.attachment_store().remove(&direct_key).await?;
                (existing, false)
            }
            _ => (direct_key, true),
        };
        let result = self
            .finalize_attachment_message(
                room_id,
                sender,
                sender_display_name,
                file_name,
                mime_type,
                size_bytes,
                is_sensitive,
                content,
                reply_to,
                content_hash,
                storage_key.clone(),
            )
            .await;
        if result.is_err() && owns_direct_object {
            let _ = self.attachment_store().remove(&storage_key).await;
        }
        result
    }
}
