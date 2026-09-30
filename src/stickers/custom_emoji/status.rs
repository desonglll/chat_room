//! Each account's emoji status: one custom emoji shown next to its name, optionally expiring.
//!
//! Reading is authorized per viewer at read time: an account sees its own status and the
//! status of accounts it shares an active chat with — the people whose names it can see in
//! a chat list, a chat header or a member list. Expired statuses are filtered in the query,
//! so nothing has to sweep them.

use chrono::{DateTime, Utc};
use sqlx::QueryBuilder;
use uuid::Uuid;

use super::models::{EmojiStatus, SetEmojiStatusRequest};
use crate::state::{with_pool, AppState};
use crate::stickers::errors::StickerError;

impl AppState {
    pub async fn set_emoji_status(
        &self,
        user_id: Uuid,
        request: &SetEmojiStatusRequest,
    ) -> Result<EmojiStatus, StickerError> {
        let now = Utc::now();
        if request
            .expires_at
            .is_some_and(|expires_at| expires_at <= now)
        {
            return Err(StickerError::Invalid("invalid_expiry"));
        }
        let emoji = self
            .resolve_custom_emoji(&[request.custom_emoji_id])
            .await?
            .remove(&request.custom_emoji_id)
            .ok_or(StickerError::Invalid("custom_emoji_not_found"))?;
        with_pool!(self, |pool| {
            sqlx::query(
                "INSERT INTO user_emoji_status (user_id, custom_emoji_id, expires_at, updated_at) \
                 VALUES ($1, $2, $3, $4) ON CONFLICT (user_id) DO UPDATE SET \
                 custom_emoji_id = excluded.custom_emoji_id, expires_at = excluded.expires_at, \
                 updated_at = excluded.updated_at",
            )
            .bind(user_id)
            .bind(request.custom_emoji_id)
            .bind(request.expires_at)
            .bind(now)
            .execute(pool)
            .await
            .map(|_| ())
        })?;
        Ok(EmojiStatus {
            user_id,
            custom_emoji_id: request.custom_emoji_id,
            expires_at: request.expires_at,
            emoji,
        })
    }

    pub async fn clear_emoji_status(&self, user_id: Uuid) -> Result<(), sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query("DELETE FROM user_emoji_status WHERE user_id = $1")
                .bind(user_id)
                .execute(pool)
                .await
                .map(|_| ())
        })
    }

    /// The unexpired statuses among `user_ids` that `viewer_id` may see, in no set order.
    /// A status whose emoji was removed from its set is omitted, like an unknown emoji.
    pub async fn emoji_statuses(
        &self,
        viewer_id: Uuid,
        user_ids: &[Uuid],
    ) -> Result<Vec<EmojiStatus>, sqlx::Error> {
        if user_ids.is_empty() {
            return Ok(Vec::new());
        }
        let now = Utc::now();
        let rows: Vec<(Uuid, Uuid, Option<DateTime<Utc>>)> = with_pool!(self, |pool| {
            let mut query = QueryBuilder::new(
                "SELECT status.user_id, status.custom_emoji_id, status.expires_at \
                 FROM user_emoji_status status \
                 WHERE (status.expires_at IS NULL OR status.expires_at > ",
            );
            query.push_bind(now);
            query.push(") AND (status.user_id = ");
            query.push_bind(viewer_id);
            query.push(
                " OR EXISTS (SELECT 1 FROM chat_members mine \
                 JOIN chat_members theirs ON theirs.room_id = mine.room_id \
                 WHERE mine.user_id = ",
            );
            query.push_bind(viewer_id);
            query.push(
                " AND mine.status = 'active' AND theirs.status = 'active' \
                 AND theirs.user_id = status.user_id)) AND status.user_id IN (",
            );
            {
                let mut values = query.separated(", ");
                for id in user_ids {
                    values.push_bind(*id);
                }
            }
            query.push(")");
            query.build_query_as().fetch_all(pool).await
        })?;
        let emoji_ids: Vec<Uuid> = rows.iter().map(|(_, emoji_id, _)| *emoji_id).collect();
        let emojis = self.resolve_custom_emoji(&emoji_ids).await?;
        Ok(rows
            .into_iter()
            .filter_map(|(user_id, custom_emoji_id, expires_at)| {
                let emoji = emojis.get(&custom_emoji_id)?.clone();
                Some(EmojiStatus {
                    user_id,
                    custom_emoji_id,
                    expires_at,
                    emoji,
                })
            })
            .collect())
    }
}
