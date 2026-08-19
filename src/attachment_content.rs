//! Content-addressed attachment persistence and reference lifecycle.

use std::{
    collections::HashMap,
    sync::{Arc, Weak},
};

use anyhow::{Context, Result};
use chrono::Utc;
use tokio::sync::{Mutex, OwnedMutexGuard};
use uuid::Uuid;

use crate::{
    message_store::NewAttachment,
    models::{Attachment, StoredMessage, User},
    state::{with_pool, AppState},
};

/// Serializes writes for the same digest inside one server process. Separate
/// processes may still publish the same deterministic key concurrently, which
/// is safe because the bytes are identical and failed metadata writes never
/// delete a content-addressed object.
#[derive(Default)]
pub(crate) struct ContentHashLocks {
    locks: Mutex<HashMap<String, Weak<Mutex<()>>>>,
}

impl ContentHashLocks {
    pub async fn lock(&self, content_hash: &str) -> OwnedMutexGuard<()> {
        let lock = {
            let mut locks = self.locks.lock().await;
            locks.retain(|_, lock| lock.strong_count() > 0);
            match locks.get(content_hash).and_then(Weak::upgrade) {
                Some(lock) => lock,
                None => {
                    let lock = Arc::new(Mutex::new(()));
                    locks.insert(content_hash.to_string(), Arc::downgrade(&lock));
                    lock
                }
            }
        };
        lock.lock_owned().await
    }
}

impl AppState {
    /// Persist a single-shot upload under its SHA-256 digest, reusing a healthy
    /// object when the same bytes were uploaded before.
    pub async fn store_attachment_message(
        &self,
        room_id: Uuid,
        sender: &User,
        sender_display_name: &str,
        upload: NewAttachment,
        content: &str,
        reply_to: Option<Uuid>,
    ) -> Result<StoredMessage> {
        let NewAttachment {
            file_name,
            mime_type,
            is_sensitive,
            mut staged,
        } = upload;
        let content_hash = self.attachment_store().hash_staged(&mut staged).await?;
        let _guard = self.content_hash_locks().lock(&content_hash).await;
        let existing_key = self.healthy_storage_key(&content_hash).await?;
        let (storage_key, size_bytes) = match existing_key {
            Some(key) => {
                let size = staged.size();
                drop(staged);
                (key, size)
            }
            None => {
                let size = self
                    .attachment_store()
                    .commit(staged, &content_hash)
                    .await?;
                (content_hash.clone(), size)
            }
        };
        self.finalize_attachment_message(
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
            storage_key,
        )
        .await
    }

    /// Content-addressed counterpart for a completed resumable upload.
    #[allow(clippy::too_many_arguments)]
    pub async fn store_chunked_attachment_message(
        &self,
        upload_id: Uuid,
        room_id: Uuid,
        sender: &User,
        sender_display_name: &str,
        file_name: String,
        mime_type: String,
        is_sensitive: bool,
        content: &str,
        reply_to: Option<Uuid>,
    ) -> Result<StoredMessage> {
        let content_hash = self.attachment_store().hash_chunked(upload_id).await?;
        let _guard = self.content_hash_locks().lock(&content_hash).await;
        let existing_key = self.healthy_storage_key(&content_hash).await?;
        let (storage_key, size_bytes) = match existing_key {
            Some(key) => {
                let size = i64::try_from(
                    self.attachment_store().chunked_upload_size(upload_id).await?,
                )
                .context("attachment is too large")?;
                self.attachment_store().discard_chunked(upload_id).await?;
                (key, size)
            }
            None => {
                let size = self
                    .attachment_store()
                    .commit_chunked(upload_id, &content_hash)
                    .await?;
                (content_hash.clone(), size)
            }
        };
        self.finalize_attachment_message(
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
            storage_key,
        )
        .await
    }

    async fn healthy_storage_key(&self, content_hash: &str) -> Result<Option<String>> {
        let key: Option<String> = with_pool!(self, |pool| {
            sqlx::query_scalar(
                "SELECT storage_key FROM attachments \
                 WHERE content_hash = $1 AND storage_key IS NOT NULL LIMIT 1",
            )
            .bind(content_hash)
            .fetch_optional(pool)
            .await
        })?;
        match key {
            Some(key) if self.attachment_store().exists(&key).await? => Ok(Some(key)),
            _ => Ok(None),
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn finalize_attachment_message(
        &self,
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
        storage_key: String,
    ) -> Result<StoredMessage> {
        let attachment_id = Uuid::new_v4();
        let access_key = Uuid::new_v4();
        let message_id = Uuid::new_v4();
        let created_at = Utc::now();
        let reply_to = self.reply_preview(room_id, reply_to).await?;
        let persisted: Result<(), sqlx::Error> = with_pool!(self, |pool| {
            async {
                let mut transaction = pool.begin().await?;
                sqlx::query(
                    "INSERT INTO attachments \
                     (id, access_key, room_id, uploader_id, file_name, mime_type, size_bytes, \
                      is_sensitive, created_at, content_hash, storage_key) \
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)",
                )
                .bind(attachment_id)
                .bind(access_key)
                .bind(room_id)
                .bind(sender.id)
                .bind(&file_name)
                .bind(&mime_type)
                .bind(size_bytes)
                .bind(is_sensitive)
                .bind(created_at)
                .bind(&content_hash)
                .bind(&storage_key)
                .execute(&mut *transaction)
                .await?;
                sqlx::query(
                    "INSERT INTO messages \
                     (id, room_id, sender_id, sender, content, attachment_id, reply_to_id, created_at) \
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
                )
                .bind(message_id)
                .bind(room_id)
                .bind(sender.id)
                .bind(sender_display_name)
                .bind(content)
                .bind(attachment_id)
                .bind(reply_to.as_ref().map(|reply| reply.message_id))
                .bind(created_at)
                .execute(&mut *transaction)
                .await?;
                sqlx::query(
                    "UPDATE attachments SET orphaned_at = NULL \
                     WHERE storage_key = $1 AND orphaned_at IS NOT NULL",
                )
                .bind(&storage_key)
                .execute(&mut *transaction)
                .await?;
                transaction.commit().await.map(|_| ())
            }
            .await
        });
        if let Err(error) = persisted {
            tracing::warn!(
                "attachment metadata transaction failed; retained content object {storage_key}: {error}"
            );
            return Err(error.into());
        }

        Ok(StoredMessage {
            id: message_id,
            room_id,
            sender_id: Some(sender.id),
            sender: sender_display_name.to_string(),
            sender_avatar: sender.avatar_emoji.clone(),
            content: content.to_string(),
            attachment: Some(Attachment {
                id: attachment_id,
                file_name,
                mime_type,
                size_bytes,
                download_url: format!("/api/attachments/{attachment_id}?key={access_key}"),
                is_sensitive,
            }),
            reply_to,
            recalled_at: None,
            edited_at: None,
            created_at,
            forwarded_from: None,
        })
    }

    pub(crate) async fn recompute_attachment_orphan_status(
        &self,
        attachment_id: Uuid,
    ) -> Result<(), sqlx::Error> {
        with_pool!(self, |pool| {
            async {
                let group_key: Option<String> = sqlx::query_scalar(
                    "SELECT COALESCE(storage_key, CAST(id AS TEXT)) FROM attachments WHERE id = $1",
                )
                .bind(attachment_id)
                .fetch_optional(pool)
                .await?;
                let Some(group_key) = group_key else {
                    return Ok(());
                };
                let referenced: bool = sqlx::query_scalar(
                    "SELECT EXISTS (\
                       SELECT 1 FROM attachments a JOIN messages m ON m.attachment_id = a.id \
                       WHERE COALESCE(a.storage_key, CAST(a.id AS TEXT)) = $1 \
                       AND m.recalled_at IS NULL\
                     )",
                )
                .bind(&group_key)
                .fetch_one(pool)
                .await?;
                let query = if referenced {
                    "UPDATE attachments SET orphaned_at = NULL \
                     WHERE COALESCE(storage_key, CAST(id AS TEXT)) = $1"
                } else {
                    "UPDATE attachments SET orphaned_at = $2 \
                     WHERE COALESCE(storage_key, CAST(id AS TEXT)) = $1 AND orphaned_at IS NULL"
                };
                let mut update = sqlx::query(query).bind(&group_key);
                if !referenced {
                    update = update.bind(Utc::now());
                }
                update.execute(pool).await?;
                Ok(())
            }
            .await
        })
    }

    /// Idempotently hash legacy UUID-keyed objects without moving them.
    pub(crate) async fn backfill_attachment_content_hashes(&self) -> Result<()> {
        let mut after: Option<Uuid> = None;
        loop {
            let ids: Vec<Uuid> = with_pool!(self, |pool| {
                match after {
                    Some(after) => {
                        sqlx::query_scalar(
                            "SELECT id FROM attachments WHERE content_hash IS NULL AND id > $1 \
                             ORDER BY id LIMIT 200",
                        )
                        .bind(after)
                        .fetch_all(pool)
                        .await
                    }
                    None => {
                        sqlx::query_scalar(
                            "SELECT id FROM attachments WHERE content_hash IS NULL ORDER BY id LIMIT 200",
                        )
                        .fetch_all(pool)
                        .await
                    }
                }
            })
            .context("load attachments pending content-hash backfill")?;
            let Some(&last) = ids.last() else {
                return Ok(());
            };
            after = Some(last);
            for id in ids {
                let key = id.simple().to_string();
                let hash = match self.attachment_store().hash_stored(&key).await {
                    Ok(hash) => hash,
                    Err(error) => {
                        tracing::warn!("backfill content hash for attachment {id} failed: {error:#}");
                        continue;
                    }
                };
                with_pool!(self, |pool| {
                    sqlx::query("UPDATE attachments SET content_hash = $1 WHERE id = $2")
                        .bind(&hash)
                        .bind(id)
                        .execute(pool)
                        .await
                        .map(|_| ())
                })
                .with_context(|| format!("store backfilled content hash for attachment {id}"))?;
            }
        }
    }
}
