//! Persisting and loading message entities.
//!
//! Entities are written in the same transaction as the text they index (message insert in
//! `messages::text_store`, edit in `messages::actions`) because live delivery polls the
//! database: a poll between two separate writes would deliver the text without them.

use std::collections::{HashMap, HashSet};

use sqlx::{FromRow, QueryBuilder};
use uuid::Uuid;

use super::entities::{sanitize_entities, shift_entities, MessageEntity};
use crate::models::StoredMessage;
use crate::state::{with_pool, AppState};

/// Insert `$entities` for `$message_id` on an open transaction/connection of either adapter.
/// Evaluates to `Result<(), sqlx::Error>`.
macro_rules! insert_message_entities {
    ($conn:expr, $message_id:expr, $entities:expr) => {{
        let mut outcome: Result<(), sqlx::Error> = Ok(());
        for (position, entity) in $entities.iter().enumerate() {
            let inserted = sqlx::query(
                "INSERT INTO message_entities (message_id, position, entity_type, \
                 offset_utf16, length_utf16, custom_emoji_id, url, user_id, language) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
            )
            .bind($message_id)
            .bind(position as i64)
            .bind(&entity.entity_type)
            .bind(entity.offset)
            .bind(entity.length)
            .bind(entity.custom_emoji_id)
            .bind(&entity.url)
            .bind(entity.user_id)
            .bind(&entity.language)
            .execute(&mut *$conn)
            .await;
            if let Err(error) = inserted {
                outcome = Err(error);
                break;
            }
        }
        outcome
    }};
}
pub(crate) use insert_message_entities;

#[derive(FromRow)]
struct EntityRow {
    message_id: Uuid,
    entity_type: String,
    offset_utf16: i64,
    length_utf16: i64,
    custom_emoji_id: Option<Uuid>,
    url: Option<String>,
    user_id: Option<Uuid>,
    language: Option<String>,
}

impl AppState {
    /// The entities a client sent with `raw` text, made valid for the stored (trimmed) text.
    /// `leading_trim` is [`super::entities::leading_trim_utf16`] of the raw text. Custom
    /// emoji must exist and not be removed from their set; a lookup failure drops them all
    /// rather than the message.
    pub(crate) async fn accept_message_entities(
        &self,
        content: &str,
        leading_trim: i64,
        entities: Vec<MessageEntity>,
    ) -> Vec<MessageEntity> {
        if entities.is_empty() {
            return entities;
        }
        let entities = sanitize_entities(content, shift_entities(entities, leading_trim));
        let ids: Vec<Uuid> = entities
            .iter()
            .filter_map(|entity| entity.custom_emoji_id)
            .collect();
        let live = match self.live_custom_emoji_ids(&ids).await {
            Ok(live) => live,
            Err(error) => {
                tracing::warn!("custom emoji lookup failed: {error}");
                HashSet::new()
            }
        };
        entities
            .into_iter()
            .filter(|entity| entity.custom_emoji_id.is_none_or(|id| live.contains(&id)))
            .collect()
    }

    /// The subset of `ids` that are custom emoji still present in their set.
    pub(crate) async fn live_custom_emoji_ids(
        &self,
        ids: &[Uuid],
    ) -> Result<HashSet<Uuid>, sqlx::Error> {
        if ids.is_empty() {
            return Ok(HashSet::new());
        }
        let rows: Vec<(Uuid,)> = with_pool!(self, |pool| {
            let mut query = QueryBuilder::new(
                "SELECT custom_emoji.sticker_id FROM custom_emoji \
                 JOIN stickers ON stickers.id = custom_emoji.sticker_id \
                 WHERE stickers.removed_at IS NULL AND custom_emoji.sticker_id IN (",
            );
            {
                let mut values = query.separated(", ");
                for id in ids {
                    values.push_bind(*id);
                }
            }
            query.push(")");
            query.build_query_as().fetch_all(pool).await
        })?;
        Ok(rows.into_iter().map(|(id,)| id).collect())
    }

    /// Entities of each message, in stored order. Messages without any are absent.
    pub(crate) async fn load_message_entities(
        &self,
        message_ids: &[Uuid],
    ) -> Result<HashMap<Uuid, Vec<MessageEntity>>, sqlx::Error> {
        let mut grouped: HashMap<Uuid, Vec<MessageEntity>> = HashMap::new();
        if message_ids.is_empty() {
            return Ok(grouped);
        }
        let rows: Vec<EntityRow> = with_pool!(self, |pool| {
            let mut query = QueryBuilder::new(
                "SELECT message_id, entity_type, offset_utf16, length_utf16, custom_emoji_id, \
                 url, user_id, language FROM message_entities WHERE message_id IN (",
            );
            {
                let mut values = query.separated(", ");
                for id in message_ids {
                    values.push_bind(*id);
                }
            }
            query.push(") ORDER BY message_id, position");
            query.build_query_as().fetch_all(pool).await
        })?;
        for row in rows {
            grouped
                .entry(row.message_id)
                .or_default()
                .push(MessageEntity {
                    entity_type: row.entity_type,
                    offset: row.offset_utf16,
                    length: row.length_utf16,
                    custom_emoji_id: row.custom_emoji_id,
                    url: row.url,
                    user_id: row.user_id,
                    language: row.language,
                });
        }
        Ok(grouped)
    }

    /// Fill `entities` on loaded messages. A message whose text this viewer cannot see (a
    /// recalled message is redacted to empty content) gets none, so entities never outlive
    /// the text they describe.
    pub(crate) async fn attach_message_entities(
        &self,
        messages: &mut [StoredMessage],
    ) -> Result<(), sqlx::Error> {
        let ids: Vec<Uuid> = messages
            .iter()
            .filter(|message| !message.content.is_empty())
            .map(|message| message.id)
            .collect();
        let mut grouped = self.load_message_entities(&ids).await?;
        for message in messages.iter_mut() {
            message.entities = grouped.remove(&message.id).unwrap_or_default();
        }
        Ok(())
    }
}
