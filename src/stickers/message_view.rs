//! Adds `media_kind` and `sticker` to loaded messages.
//!
//! Called from `attach_message_reactions`, the one post-load step that every history,
//! replay, live-delivery, search and single-message loader already runs, so every path that
//! returns a `StoredMessage` carries the sticker fields without touching those loaders.

use std::collections::HashMap;

use sqlx::{FromRow, QueryBuilder};
use uuid::Uuid;

use super::models::{MessageSticker, StickerFormat};
use crate::models::StoredMessage;
use crate::state::{with_pool, AppState};

#[derive(FromRow)]
struct MediaRow {
    message_id: Uuid,
    media_kind: String,
    sticker_id: Option<Uuid>,
    set_id: Option<Uuid>,
    set_short_name: Option<String>,
    emoji: Option<String>,
    format: Option<String>,
    width: Option<i64>,
    height: Option<i64>,
}

impl AppState {
    pub(crate) async fn attach_message_media(
        &self,
        messages: &mut [StoredMessage],
    ) -> Result<(), sqlx::Error> {
        // Only attachment-bearing messages can be stickers; a recalled message has already
        // lost its attachment for this viewer, and so loses its sticker too.
        let ids: Vec<Uuid> = messages
            .iter()
            .filter(|message| message.attachment.is_some())
            .map(|message| message.id)
            .collect();
        if ids.is_empty() {
            return Ok(());
        }
        let rows: Vec<MediaRow> = with_pool!(self, |pool| {
            let mut query = QueryBuilder::new(
                "SELECT messages.id AS message_id, messages.media_kind, \
                 stickers.id AS sticker_id, stickers.set_id, \
                 sticker_sets.short_name AS set_short_name, stickers.emoji, stickers.format, \
                 stickers.width, stickers.height FROM messages \
                 LEFT JOIN stickers ON stickers.id = messages.sticker_id \
                 LEFT JOIN sticker_sets ON sticker_sets.id = stickers.set_id \
                 WHERE messages.media_kind IS NOT NULL AND messages.id IN (",
            );
            {
                let mut values = query.separated(", ");
                for id in &ids {
                    values.push_bind(*id);
                }
            }
            query.push(")");
            query.build_query_as().fetch_all(pool).await
        })?;
        let mut by_message: HashMap<Uuid, MediaRow> =
            rows.into_iter().map(|row| (row.message_id, row)).collect();
        for message in messages.iter_mut() {
            let Some(row) = by_message.remove(&message.id) else {
                continue;
            };
            message.sticker = row.sticker();
            message.media_kind = Some(row.media_kind);
        }
        Ok(())
    }
}

impl MediaRow {
    fn sticker(&self) -> Option<MessageSticker> {
        Some(MessageSticker {
            sticker_id: self.sticker_id?,
            set_id: self.set_id?,
            set_short_name: self.set_short_name.clone()?,
            emoji: self.emoji.clone()?,
            format: StickerFormat::parse(self.format.as_deref()?)?,
            width: self.width?,
            height: self.height?,
        })
    }
}
