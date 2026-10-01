//! Adds a channel post's `views` and `post_author` to loaded messages (TG-202).
//!
//! Called from `attach_message_reactions`, the one post-load step every history, replay,
//! live-delivery, search and single-message loader already runs (the TG-302 pattern), so every
//! path that returns a `StoredMessage` carries the channel fields without touching the loaders.
//! Messages outside channels are left as they are (`views` absent = "not a channel post").

use std::collections::{HashMap, HashSet};

use sqlx::{FromRow, QueryBuilder};
use uuid::Uuid;

use super::ChatType;
use crate::models::StoredMessage;
use crate::state::{with_pool, AppState};

#[derive(FromRow)]
struct PostRow {
    id: Uuid,
    views_count: i64,
    post_author: Option<String>,
}

impl AppState {
    pub(crate) async fn attach_channel_post_fields(
        &self,
        messages: &mut [StoredMessage],
    ) -> Result<(), sqlx::Error> {
        let rooms: HashSet<Uuid> = messages.iter().map(|message| message.room_id).collect();
        let mut channels = HashSet::new();
        for room_id in rooms {
            if self
                .chat(room_id)
                .await
                .is_some_and(|chat| chat.chat_type == ChatType::Channel)
            {
                channels.insert(room_id);
            }
        }
        let ids: Vec<Uuid> = messages
            .iter()
            .filter(|message| channels.contains(&message.room_id))
            .map(|message| message.id)
            .collect();
        if ids.is_empty() {
            return Ok(());
        }
        let rows: Vec<PostRow> = with_pool!(self, |pool| {
            let mut query = QueryBuilder::new(
                "SELECT id, CAST(views_count AS BIGINT) AS views_count, post_author \
                 FROM messages WHERE id IN (",
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
        let mut by_id: HashMap<Uuid, PostRow> = rows.into_iter().map(|row| (row.id, row)).collect();
        // TG-203: and the comment count of every post that has a thread.
        let comments = self.comment_counts(&ids).await?;
        for message in messages.iter_mut() {
            if let Some(row) = by_id.remove(&message.id) {
                message.views = Some(row.views_count);
                message.post_author = row.post_author;
                message.comments = comments.get(&message.id).copied();
            }
        }
        Ok(())
    }
}
