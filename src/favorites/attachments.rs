use anyhow::Result;
use chrono::Utc;
use uuid::Uuid;

use crate::favorites::models::FavoriteItem;
use crate::message_store::NewAttachment;
use crate::state::{with_pool, AppState};

impl AppState {
    pub async fn create_attachment_favorite(
        &self,
        user_id: Uuid,
        title: &str,
        content: &str,
        upload: NewAttachment,
    ) -> Result<FavoriteItem> {
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
        let attachment_id = Uuid::new_v4();
        let favorite_id = Uuid::new_v4();
        let access_key = Uuid::new_v4();
        let now = Utc::now();
        with_pool!(self, |pool| {
            let mut transaction = pool.begin().await?;
            sqlx::query(
                "INSERT INTO attachments \
                 (id, access_key, room_id, uploader_id, file_name, mime_type, size_bytes, \
                  is_sensitive, created_at, content_hash, storage_key) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)",
            )
            .bind(attachment_id)
            .bind(access_key)
            .bind(None::<Uuid>)
            .bind(user_id)
            .bind(file_name)
            .bind(mime_type)
            .bind(size_bytes)
            .bind(is_sensitive)
            .bind(now)
            .bind(&content_hash)
            .bind(&storage_key)
            .execute(&mut *transaction)
            .await?;
            sqlx::query(
                "INSERT INTO favorites \
                 (id, user_id, kind, title, content, attachment_id, created_at, updated_at) \
                 VALUES ($1, $2, 'manual', $3, $4, $5, $6, $7)",
            )
            .bind(favorite_id)
            .bind(user_id)
            .bind(title)
            .bind(content)
            .bind(attachment_id)
            .bind(now)
            .bind(now)
            .execute(&mut *transaction)
            .await?;
            sqlx::query(
                "UPDATE attachments SET orphaned_at = NULL \
                 WHERE storage_key = $1 AND orphaned_at IS NOT NULL",
            )
            .bind(storage_key)
            .execute(&mut *transaction)
            .await?;
            transaction.commit().await?;
            Ok::<_, sqlx::Error>(())
        })?;
        self.favorite_by_id(user_id, favorite_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("created favorite was not found"))
    }
}
