use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::models::User;
use crate::state::{with_pool, AppState};

pub(crate) struct AvatarFile {
    pub storage_key: String,
    pub mime_type: String,
    pub size_bytes: i64,
    pub updated_at: DateTime<Utc>,
}

impl AppState {
    pub(crate) async fn avatar_file(
        &self,
        user_id: Uuid,
    ) -> Result<Option<AvatarFile>, sqlx::Error> {
        #[derive(sqlx::FromRow)]
        struct Row {
            storage_key: String,
            mime_type: String,
            size_bytes: i64,
            updated_at: DateTime<Utc>,
        }
        with_pool!(self, |pool| {
            sqlx::query_as(
                "SELECT storage_key, mime_type, size_bytes, updated_at \
                 FROM user_avatar_files WHERE user_id = $1",
            )
            .bind(user_id)
            .fetch_optional(pool)
            .await
            .map(|row: Option<Row>| {
                row.map(|row| AvatarFile {
                    storage_key: row.storage_key,
                    mime_type: row.mime_type,
                    size_bytes: row.size_bytes,
                    updated_at: row.updated_at,
                })
            })
        })
    }

    pub(crate) async fn replace_user_avatar_file(
        &self,
        user_id: Uuid,
        storage_key: &str,
        mime_type: &str,
        size_bytes: i64,
        avatar_url: &str,
    ) -> Result<Option<(Option<String>, User)>, sqlx::Error> {
        let now = Utc::now();
        let result = with_pool!(self, |pool| {
            let mut transaction = pool.begin().await?;
            let old_key: Option<String> =
                sqlx::query_scalar("SELECT storage_key FROM user_avatar_files WHERE user_id = $1")
                    .bind(user_id)
                    .fetch_optional(&mut *transaction)
                    .await?;
            sqlx::query(
                "INSERT INTO user_avatar_files \
                 (user_id, storage_key, mime_type, size_bytes, updated_at) \
                 VALUES ($1, $2, $3, $4, $5) ON CONFLICT(user_id) DO UPDATE SET \
                 storage_key = excluded.storage_key, mime_type = excluded.mime_type, \
                 size_bytes = excluded.size_bytes, updated_at = excluded.updated_at",
            )
            .bind(user_id)
            .bind(storage_key)
            .bind(mime_type)
            .bind(size_bytes)
            .bind(now)
            .execute(&mut *transaction)
            .await?;
            let user: Option<User> = sqlx::query_as(
                "UPDATE users SET avatar_emoji = $1 WHERE id = $2 \
                 RETURNING id, username, avatar_emoji, display_name, signature, homepage, created_at",
            )
            .bind(avatar_url)
            .bind(user_id)
            .fetch_optional(&mut *transaction)
            .await?;
            transaction.commit().await?;
            Ok::<_, sqlx::Error>(user.map(|user| (old_key, user)))
        })?;
        if result.is_some() {
            self.invalidate_user_sessions(user_id).await;
        }
        Ok(result)
    }

    pub(crate) async fn delete_user_avatar_file(
        &self,
        user_id: Uuid,
    ) -> Result<Option<String>, sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query_scalar(
                "DELETE FROM user_avatar_files WHERE user_id = $1 RETURNING storage_key",
            )
            .bind(user_id)
            .fetch_optional(pool)
            .await
        })
    }
}
