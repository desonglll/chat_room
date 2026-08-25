use chrono::{DateTime, Utc};
use sqlx::FromRow;
use uuid::Uuid;

use crate::favorites::models::FavoriteItem;
use crate::models::{Attachment, StoredMessage, User};
use crate::state::{with_pool, AppState};

const FAVORITE_SELECT: &str = "SELECT favorites.id, favorites.kind, favorites.title, \
    favorites.content, favorites.source_message_id, \
    CASE WHEN source_membership.user_id IS NULL THEN NULL ELSE source_message.room_id END AS source_room_id, \
    favorites.source_sender, \
    favorites.source_room_name, favorites.created_at, favorites.updated_at, \
    attachments.id AS attachment_id, attachments.access_key AS attachment_access_key, \
    attachments.file_name AS attachment_file_name, attachments.mime_type AS attachment_mime_type, \
    attachments.size_bytes AS attachment_size_bytes, \
    attachments.is_sensitive AS attachment_is_sensitive FROM favorites \
    LEFT JOIN attachments ON attachments.id = favorites.attachment_id \
    LEFT JOIN messages AS source_message ON source_message.id = favorites.source_message_id \
    LEFT JOIN rooms AS source_room ON source_room.id = source_message.room_id AND source_room.deleted_at IS NULL \
    LEFT JOIN room_memberships AS source_membership ON source_membership.room_id = source_room.id \
      AND source_membership.user_id = favorites.user_id AND source_membership.status = 'active'";

#[derive(FromRow)]
struct FavoriteRow {
    id: Uuid,
    kind: String,
    title: String,
    content: String,
    source_message_id: Option<Uuid>,
    source_room_id: Option<Uuid>,
    source_sender: String,
    source_room_name: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    attachment_id: Option<Uuid>,
    attachment_access_key: Option<Uuid>,
    attachment_file_name: Option<String>,
    attachment_mime_type: Option<String>,
    attachment_size_bytes: Option<i64>,
    attachment_is_sensitive: Option<bool>,
}

impl FavoriteRow {
    fn into_item(self) -> FavoriteItem {
        let attachment = self.attachment_id.and_then(|id| {
            Some(Attachment {
                id,
                file_name: self.attachment_file_name?,
                mime_type: self.attachment_mime_type?,
                size_bytes: self.attachment_size_bytes?,
                download_url: format!("/api/attachments/{id}?key={}", self.attachment_access_key?),
                is_sensitive: self.attachment_is_sensitive.unwrap_or(false),
            })
        });
        FavoriteItem {
            id: self.id,
            kind: self.kind,
            title: self.title,
            content: self.content,
            source_message_id: self.source_message_id,
            source_room_id: self.source_room_id,
            source_sender: self.source_sender,
            source_room_name: self.source_room_name,
            attachment,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

impl AppState {
    pub(crate) async fn favorite_attachment_ids(
        &self,
        user_id: Uuid,
    ) -> Result<Vec<Uuid>, sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query_scalar(
                "SELECT DISTINCT attachment_id FROM favorites \
                 WHERE user_id = $1 AND attachment_id IS NOT NULL",
            )
            .bind(user_id)
            .fetch_all(pool)
            .await
        })
    }

    pub async fn favorites(&self, user_id: Uuid) -> Result<Vec<FavoriteItem>, sqlx::Error> {
        let query = format!(
            "{FAVORITE_SELECT} WHERE favorites.user_id = $1 \
             ORDER BY favorites.created_at DESC, favorites.id DESC LIMIT 500"
        );
        let rows: Vec<FavoriteRow> = with_pool!(self, |pool| {
            sqlx::query_as(&query).bind(user_id).fetch_all(pool).await
        })?;
        Ok(rows.into_iter().map(FavoriteRow::into_item).collect())
    }

    pub async fn favorite_by_id(
        &self,
        user_id: Uuid,
        favorite_id: Uuid,
    ) -> Result<Option<FavoriteItem>, sqlx::Error> {
        let query = format!("{FAVORITE_SELECT} WHERE favorites.user_id = $1 AND favorites.id = $2");
        let row: Option<FavoriteRow> = with_pool!(self, |pool| {
            sqlx::query_as(&query)
                .bind(user_id)
                .bind(favorite_id)
                .fetch_optional(pool)
                .await
        })?;
        Ok(row.map(FavoriteRow::into_item))
    }

    pub async fn create_manual_favorite(
        &self,
        user_id: Uuid,
        title: &str,
        content: &str,
    ) -> Result<FavoriteItem, sqlx::Error> {
        let id = Uuid::new_v4();
        let now = Utc::now();
        with_pool!(self, |pool| {
            sqlx::query(
                "INSERT INTO favorites (id, user_id, kind, title, content, created_at, updated_at) \
                 VALUES ($1, $2, 'manual', $3, $4, $5, $6)",
            )
            .bind(id)
            .bind(user_id)
            .bind(title)
            .bind(content)
            .bind(now)
            .bind(now)
            .execute(pool)
            .await
            .map(|_| ())
        })?;
        self.favorite_by_id(user_id, id)
            .await?
            .ok_or(sqlx::Error::RowNotFound)
    }

    pub async fn favorite_message(
        &self,
        user_id: Uuid,
        message_id: Uuid,
    ) -> Result<Option<FavoriteItem>, sqlx::Error> {
        let id = Uuid::new_v4();
        let now = Utc::now();
        let attachment_id: Option<Uuid> = with_pool!(self, |pool| {
            let mut transaction = pool.begin().await?;
            sqlx::query(
                "INSERT INTO favorites \
                 (id, user_id, source_message_id, kind, title, content, source_sender, \
                  source_room_name, attachment_id, created_at, updated_at) \
                 SELECT $1, $2, messages.id, \
                   CASE WHEN attachments.mime_type LIKE 'video/%' THEN 'video' ELSE 'message' END, \
                   CASE WHEN attachments.mime_type LIKE 'video/%' THEN attachments.file_name ELSE '' END, \
                   messages.content, messages.sender, \
                   CASE WHEN direct.room_id IS NULL THEN rooms.name \
                     ELSE COALESCE(NULLIF(peer.display_name, ''), peer.username) END, \
                   messages.attachment_id, $4, $4 \
                 FROM messages JOIN rooms ON rooms.id = messages.room_id \
                 LEFT JOIN attachments ON attachments.id = messages.attachment_id \
                 LEFT JOIN direct_conversations AS direct ON direct.room_id = rooms.id \
                 LEFT JOIN users AS peer ON peer.id = CASE \
                   WHEN direct.user_low_id = $2 THEN direct.user_high_id \
                   WHEN direct.user_high_id = $2 THEN direct.user_low_id ELSE NULL END \
                 WHERE messages.id = $3 AND messages.recalled_at IS NULL \
                   AND EXISTS (SELECT 1 FROM room_memberships \
                     WHERE room_memberships.room_id = messages.room_id \
                       AND room_memberships.user_id = $2 AND room_memberships.status = 'active') \
                 ON CONFLICT DO NOTHING",
            )
            .bind(id)
            .bind(user_id)
            .bind(message_id)
            .bind(now)
            .execute(&mut *transaction)
            .await?;
            let attachment_id: Option<Uuid> = sqlx::query_scalar(
                "SELECT attachment_id FROM favorites \
                 WHERE user_id = $1 AND source_message_id = $2",
            )
            .bind(user_id)
            .bind(message_id)
            .fetch_optional(&mut *transaction)
            .await?
            .flatten();
            if let Some(attachment_id) = attachment_id {
                sqlx::query(
                    "UPDATE attachments SET orphaned_at = NULL WHERE \
                     COALESCE(storage_key, CAST(id AS TEXT)) = \
                     (SELECT COALESCE(storage_key, CAST(id AS TEXT)) FROM attachments WHERE id = $1)",
                )
                .bind(attachment_id)
                .execute(&mut *transaction)
                .await?;
            }
            transaction.commit().await?;
            Ok::<_, sqlx::Error>(attachment_id)
        })?;
        let _ = attachment_id;
        let query = format!(
            "{FAVORITE_SELECT} WHERE favorites.user_id = $1 \
             AND favorites.source_message_id = $2"
        );
        let row: Option<FavoriteRow> = with_pool!(self, |pool| {
            sqlx::query_as(&query)
                .bind(user_id)
                .bind(message_id)
                .fetch_optional(pool)
                .await
        })?;
        Ok(row.map(FavoriteRow::into_item))
    }

    pub async fn delete_favorite(
        &self,
        user_id: Uuid,
        favorite_id: Uuid,
    ) -> Result<(bool, Option<Uuid>), sqlx::Error> {
        let attachment_id: Option<Option<Uuid>> = with_pool!(self, |pool| {
            sqlx::query_scalar(
                "DELETE FROM favorites WHERE id = $1 AND user_id = $2 RETURNING attachment_id",
            )
            .bind(favorite_id)
            .bind(user_id)
            .fetch_optional(pool)
            .await
        })?;
        if let Some(Some(attachment_id)) = attachment_id {
            self.recompute_attachment_orphan_status(attachment_id)
                .await?;
        }
        Ok((attachment_id.is_some(), attachment_id.flatten()))
    }

    pub async fn forward_favorite(
        &self,
        favorite_id: Uuid,
        target_room_id: Uuid,
        forwarder: &User,
    ) -> Result<Option<StoredMessage>, sqlx::Error> {
        let id = Uuid::new_v4();
        let now = Utc::now();
        let display_name = self.resolve_display_name(target_room_id, forwarder).await;
        let inserted = with_pool!(self, |pool| {
            sqlx::query(
                "INSERT INTO messages \
                 (id, room_id, sender_id, sender, content, attachment_id, \
                  forwarded_from_sender, forwarded_from_room_name, created_at) \
                 SELECT $1, $2, $3, $4, \
                   CASE WHEN favorites.kind = 'manual' AND favorites.content = '' \
                     THEN favorites.title ELSE favorites.content END, favorites.attachment_id, \
                   CASE WHEN favorites.kind = 'manual' THEN '我的收藏' \
                     ELSE favorites.source_sender END, \
                   CASE WHEN favorites.kind = 'manual' THEN '个人收藏' \
                     ELSE favorites.source_room_name END, $6 \
                 FROM favorites WHERE favorites.id = $5 AND favorites.user_id = $3 \
                   AND EXISTS (SELECT 1 FROM room_memberships \
                     JOIN room_role_permissions ON room_role_permissions.role_id = room_memberships.role_id \
                     JOIN rooms ON rooms.id = room_memberships.room_id AND rooms.deleted_at IS NULL \
                     WHERE room_memberships.room_id = $2 AND room_memberships.user_id = $3 \
                       AND room_memberships.status = 'active' \
                       AND room_role_permissions.permission_key = 'message.send')",
            )
            .bind(id)
            .bind(target_room_id)
            .bind(forwarder.id)
            .bind(display_name)
            .bind(favorite_id)
            .bind(now)
            .execute(pool)
            .await
            .map(|result| result.rows_affected() > 0)
        })?;
        if !inserted {
            return Ok(None);
        }
        self.message_by_id(id, Some(forwarder.id)).await
    }
}
