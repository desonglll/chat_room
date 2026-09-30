//! Phase two of sending an album, and the album-wide recall: each is ONE transaction.

use chrono::{DateTime, Duration, Utc};
use uuid::Uuid;

use super::model::AlbumError;
use super::promote::PromotedItem;
use crate::models::{Attachment, ReplyPreview, StoredMessage, User};
use crate::state::{with_pool, AppState};

/// Everything the album transaction writes besides the items themselves.
pub(super) struct NewAlbum<'a> {
    pub room_id: Uuid,
    pub sender: &'a User,
    pub sender_display_name: &'a str,
    pub caption: &'a str,
    pub reply_to: Option<ReplyPreview>,
    pub is_sensitive: bool,
    pub silent: bool,
    /// TG-204: already resolved by `resolve_post_topic`; `None` = General.
    pub topic_id: Option<Uuid>,
}

struct Row {
    message_id: Uuid,
    attachment_id: Uuid,
    access_key: Uuid,
    created_at: DateTime<Utc>,
}

impl AppState {
    /// Claim every upload session, insert one attachment + one message per item with a shared
    /// `grouped_id`, and commit — or roll everything back. Items get strictly increasing
    /// `created_at` (1 µs apart), so history order (`created_at, id`) is album order.
    pub(super) async fn insert_album(
        &self,
        album: NewAlbum<'_>,
        items: &[PromotedItem],
    ) -> Result<(Uuid, Vec<StoredMessage>), AlbumError> {
        let grouped_id = Uuid::new_v4();
        let now = Utc::now();
        let base = now - Duration::nanoseconds(i64::from(now.timestamp_subsec_nanos() % 1_000));
        let rows: Vec<Row> = (0..items.len())
            .map(|index| Row {
                message_id: Uuid::new_v4(),
                attachment_id: Uuid::new_v4(),
                access_key: Uuid::new_v4(),
                created_at: base + Duration::microseconds(index as i64),
            })
            .collect();
        let sender_id = album.sender.id;
        let reply_id = album.reply_to.as_ref().map(|reply| reply.message_id);
        let outcome: Result<(), AlbumError> = with_pool!(self, |pool| {
            async {
                let mut tx = pool.begin().await?;
                let allowed: bool = sqlx::query_scalar(
                    "SELECT EXISTS(SELECT 1 FROM chat_members \
                     JOIN chat_role_permissions ON chat_role_permissions.role_id = chat_members.role_id \
                     WHERE chat_members.room_id = $1 AND chat_members.user_id = $2 \
                       AND chat_members.status = 'active' \
                       AND chat_role_permissions.permission_key = 'message.send')",
                )
                .bind(album.room_id)
                .bind(sender_id)
                .fetch_one(&mut *tx)
                .await?;
                if !allowed {
                    return Ok(Err(AlbumError::Forbidden));
                }
                for item in items {
                    // The claim is what makes a double submit (two tabs, a retried request)
                    // produce exactly one album: the loser matches no row and rolls back.
                    let claimed = sqlx::query(
                        "UPDATE attachment_uploads SET status = 'completed', updated_at = $1 \
                         WHERE id = $2 AND uploader_id = $3 AND room_id = $4 \
                           AND status = 'in_progress'",
                    )
                    .bind(now)
                    .bind(item.upload_id)
                    .bind(sender_id)
                    .bind(album.room_id)
                    .execute(&mut *tx)
                    .await?
                    .rows_affected();
                    if claimed != 1 {
                        return Ok(Err(AlbumError::Conflict));
                    }
                }
                for (index, (item, row)) in items.iter().zip(&rows).enumerate() {
                    sqlx::query(
                        "INSERT INTO attachments \
                         (id, access_key, room_id, uploader_id, file_name, mime_type, size_bytes, \
                          is_sensitive, created_at, content_hash, storage_key) \
                         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)",
                    )
                    .bind(row.attachment_id)
                    .bind(row.access_key)
                    .bind(album.room_id)
                    .bind(sender_id)
                    .bind(&item.file_name)
                    .bind(&item.mime_type)
                    .bind(item.size_bytes)
                    .bind(album.is_sensitive)
                    .bind(row.created_at)
                    .bind(&item.content_hash)
                    .bind(&item.storage_key)
                    .execute(&mut *tx)
                    .await?;
                    sqlx::query(
                        "INSERT INTO messages \
                         (id, room_id, sender_id, sender, content, attachment_id, reply_to_id, \
                          created_at, silent, grouped_id, topic_id) \
                         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)",
                    )
                    .bind(row.message_id)
                    .bind(album.room_id)
                    .bind(sender_id)
                    .bind(album.sender_display_name)
                    .bind(if index == 0 { album.caption } else { "" })
                    .bind(row.attachment_id)
                    .bind(if index == 0 { reply_id } else { None })
                    .bind(row.created_at)
                    .bind(album.silent)
                    .bind(grouped_id)
                    .bind(album.topic_id)
                    .execute(&mut *tx)
                    .await?;
                    sqlx::query(
                        "UPDATE attachments SET orphaned_at = NULL \
                         WHERE storage_key = $1 AND orphaned_at IS NOT NULL",
                    )
                    .bind(&item.storage_key)
                    .execute(&mut *tx)
                    .await?;
                }
                tx.commit().await?;
                Ok::<Result<(), AlbumError>, sqlx::Error>(Ok(()))
            }
            .await
        })?;
        outcome?;

        let messages = items
            .iter()
            .zip(rows)
            .enumerate()
            .map(|(index, (item, row))| StoredMessage {
                id: row.message_id,
                room_id: album.room_id,
                sender_id: Some(sender_id),
                sender: album.sender_display_name.to_string(),
                sender_avatar: album.sender.avatar_emoji.clone(),
                content: if index == 0 {
                    album.caption.to_string()
                } else {
                    String::new()
                },
                attachment: Some(Attachment {
                    id: row.attachment_id,
                    file_name: item.file_name.clone(),
                    mime_type: item.mime_type.clone(),
                    size_bytes: item.size_bytes,
                    download_url: format!(
                        "/api/attachments/{}?key={}",
                        row.attachment_id, row.access_key
                    ),
                    is_sensitive: album.is_sensitive,
                }),
                reply_to: if index == 0 {
                    album.reply_to.clone()
                } else {
                    None
                },
                created_at: row.created_at,
                silent: album.silent,
                grouped_id: Some(grouped_id),
                ..Default::default()
            })
            .collect();
        Ok((grouped_id, messages))
    }

    /// Recall every still-visible item `sender_id` sent in the album, in one statement.
    /// Returns `(message_id, attachment_id)` of each recalled item in album order.
    pub(super) async fn recall_album_items(
        &self,
        room_id: Uuid,
        sender_id: Uuid,
        grouped_id: Uuid,
        recalled_at: DateTime<Utc>,
    ) -> Result<Vec<(Uuid, Option<Uuid>)>, sqlx::Error> {
        let mut rows: Vec<(Uuid, Option<Uuid>, DateTime<Utc>)> = with_pool!(self, |pool| {
            sqlx::query_as(
                "UPDATE messages SET recalled_at = $1 \
                 WHERE room_id = $2 AND sender_id = $3 AND grouped_id = $4 \
                   AND recalled_at IS NULL \
                 RETURNING id, attachment_id, created_at",
            )
            .bind(recalled_at)
            .bind(room_id)
            .bind(sender_id)
            .bind(grouped_id)
            .fetch_all(pool)
            .await
        })?;
        rows.sort_by_key(|(id, _, created_at)| (*created_at, *id));
        Ok(rows
            .into_iter()
            .map(|(id, attachment_id, _)| (id, attachment_id))
            .collect())
    }
}
