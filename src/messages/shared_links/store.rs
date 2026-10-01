//! The lazy link index (`message_links` + `chat_link_index_state`) and its paged read.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use sqlx::QueryBuilder;
use uuid::Uuid;

use super::{extract_urls, SharedLinkItem};
use crate::state::{with_pool, AppState};

/// Messages extracted per batch, and batches per refresh: a chat with a long history is
/// backfilled over a few reads instead of stalling one request.
const BATCH: i64 = 500;
const MAX_BATCHES: usize = 20;

type IndexState = (DateTime<Utc>, Uuid, DateTime<Utc>);

#[derive(sqlx::FromRow)]
struct SourceRow {
    id: Uuid,
    content: String,
    created_at: DateTime<Utc>,
    recalled: bool,
}

impl AppState {
    /// Bring `room_id`'s link index up to date: re-extract messages edited since the last
    /// refresh, then extract messages after the cursor (in bounded batches).
    pub(crate) async fn refresh_link_index(&self, room_id: Uuid) -> Result<(), sqlx::Error> {
        // Edits after this instant are picked up by the next refresh.
        let started = Utc::now();
        let state: Option<IndexState> = with_pool!(self, |pool| {
            sqlx::query_as(
                "SELECT indexed_created_at, indexed_message_id, edits_seen_at \
                 FROM chat_link_index_state WHERE room_id = $1",
            )
            .bind(room_id)
            .fetch_optional(pool)
            .await
        })?;
        let mut cursor = state.map(|(created_at, id, _)| (created_at, id));
        if let Some((created_at, id, edits_seen_at)) = state {
            let edited: Vec<SourceRow> = with_pool!(self, |pool| {
                sqlx::query_as(
                    "SELECT id, content, created_at, recalled_at IS NOT NULL AS recalled \
                     FROM messages WHERE room_id = $1 AND edited_at > $2 \
                       AND (created_at < $3 OR (created_at = $3 AND id <= $4))",
                )
                .bind(room_id)
                .bind(edits_seen_at)
                .bind(created_at)
                .bind(id)
                .fetch_all(pool)
                .await
            })?;
            if !edited.is_empty() {
                self.write_links(room_id, &edited, None, started).await?;
            }
        }
        for _ in 0..MAX_BATCHES {
            let rows: Vec<SourceRow> =
                with_pool!(self, |pool| {
                    match cursor {
                        Some((created_at, id)) => sqlx::query_as(
                            "SELECT id, content, created_at, recalled_at IS NOT NULL AS recalled \
                             FROM messages WHERE room_id = $1 \
                               AND (created_at > $2 OR (created_at = $2 AND id > $3)) \
                             ORDER BY created_at, id LIMIT $4",
                        )
                        .bind(room_id)
                        .bind(created_at)
                        .bind(id)
                        .bind(BATCH)
                        .fetch_all(pool)
                        .await,
                        None => sqlx::query_as(
                            "SELECT id, content, created_at, recalled_at IS NOT NULL AS recalled \
                             FROM messages WHERE room_id = $1 ORDER BY created_at, id LIMIT $2",
                        )
                        .bind(room_id)
                        .bind(BATCH)
                        .fetch_all(pool)
                        .await,
                    }
                })?;
            let Some(last) = rows.last() else {
                if cursor.is_none() {
                    // An empty chat: nothing to index yet, and no state to keep.
                    return Ok(());
                }
                break;
            };
            let next = (last.created_at, last.id);
            let full = rows.len() as i64 == BATCH;
            self.write_links(room_id, &rows, Some(next), started)
                .await?;
            cursor = Some(next);
            if !full {
                break;
            }
        }
        Ok(())
    }

    /// Replace the links of `rows` and, when `advance_to` is set, move the cursor — in one
    /// transaction, so a concurrent refresh can at worst redo a batch (inserts are idempotent).
    async fn write_links(
        &self,
        room_id: Uuid,
        rows: &[SourceRow],
        advance_to: Option<(DateTime<Utc>, Uuid)>,
        edits_seen_at: DateTime<Utc>,
    ) -> Result<(), sqlx::Error> {
        let live: Vec<&SourceRow> = rows.iter().filter(|row| !row.recalled).collect();
        let text_links = self.text_link_targets(&live).await?;
        let mut links: Vec<(Uuid, i32, String, DateTime<Utc>)> = Vec::new();
        for row in &live {
            let hidden = text_links
                .get(&row.id)
                .map(|urls| urls.iter().map(String::as_str));
            let urls = extract_urls(&row.content, hidden.into_iter().flatten());
            for (position, url) in urls.into_iter().enumerate() {
                links.push((row.id, position as i32, url, row.created_at));
            }
        }
        with_pool!(self, |pool| {
            async {
                let mut transaction = pool.begin().await?;
                let mut delete = QueryBuilder::new("DELETE FROM message_links WHERE message_id IN (");
                {
                    let mut ids = delete.separated(", ");
                    for row in rows {
                        ids.push_bind(row.id);
                    }
                }
                delete.push(")");
                delete.build().execute(&mut *transaction).await?;
                if !links.is_empty() {
                    let mut insert = QueryBuilder::new(
                        "INSERT INTO message_links (message_id, position, room_id, url, created_at) ",
                    );
                    insert.push_values(&links, |mut values, (id, position, url, created_at)| {
                        values
                            .push_bind(*id)
                            .push_bind(*position)
                            .push_bind(room_id)
                            .push_bind(url.clone())
                            .push_bind(*created_at);
                    });
                    insert.push(" ON CONFLICT (message_id, position) DO NOTHING");
                    insert.build().execute(&mut *transaction).await?;
                }
                match advance_to {
                    // The cursor only moves forward, whichever refresh commits last.
                    Some((created_at, id)) => {
                        sqlx::query(
                            "INSERT INTO chat_link_index_state \
                             (room_id, indexed_created_at, indexed_message_id, edits_seen_at) \
                             VALUES ($1, $2, $3, $4) \
                             ON CONFLICT (room_id) DO UPDATE SET \
                               indexed_created_at = excluded.indexed_created_at, \
                               indexed_message_id = excluded.indexed_message_id, \
                               edits_seen_at = excluded.edits_seen_at \
                             WHERE excluded.indexed_created_at > chat_link_index_state.indexed_created_at \
                               OR (excluded.indexed_created_at = chat_link_index_state.indexed_created_at \
                                 AND excluded.indexed_message_id > chat_link_index_state.indexed_message_id)",
                        )
                        .bind(room_id)
                        .bind(created_at)
                        .bind(id)
                        .bind(edits_seen_at)
                        .execute(&mut *transaction)
                        .await?;
                    }
                    None => {
                        sqlx::query(
                            "UPDATE chat_link_index_state SET edits_seen_at = $2 \
                             WHERE room_id = $1 AND edits_seen_at < $2",
                        )
                        .bind(room_id)
                        .bind(edits_seen_at)
                        .execute(&mut *transaction)
                        .await?;
                    }
                }
                transaction.commit().await
            }
            .await
        })
    }

    async fn text_link_targets(
        &self,
        rows: &[&SourceRow],
    ) -> Result<HashMap<Uuid, Vec<String>>, sqlx::Error> {
        if rows.is_empty() {
            return Ok(HashMap::new());
        }
        let found: Vec<(Uuid, String)> = with_pool!(self, |pool| {
            let mut query = QueryBuilder::new(
                "SELECT message_id, url FROM message_entities \
                 WHERE entity_type = 'text_link' AND url IS NOT NULL AND message_id IN (",
            );
            {
                let mut ids = query.separated(", ");
                for row in rows {
                    ids.push_bind(row.id);
                }
            }
            query.push(") ORDER BY message_id, position");
            query.build_query_as().fetch_all(pool).await
        })?;
        let mut by_message: HashMap<Uuid, Vec<String>> = HashMap::new();
        for (id, url) in found {
            by_message.entry(id).or_default().push(url);
        }
        Ok(by_message)
    }

    /// One page of `room_id`'s links, newest first, recalled messages excluded. `before` is
    /// the last item of the previous page.
    pub(crate) async fn shared_links_page(
        &self,
        room_id: Uuid,
        before: Option<(DateTime<Utc>, Uuid, i32)>,
        limit: i64,
    ) -> Result<Vec<SharedLinkItem>, sqlx::Error> {
        with_pool!(self, |pool| {
            match before {
                    Some((created_at, id, position)) => {
                        sqlx::query_as(
                            "SELECT links.message_id, links.position, links.url, messages.sender_id, \
                             messages.sender, links.created_at FROM message_links AS links \
                             JOIN messages ON messages.id = links.message_id \
                             WHERE links.room_id = $1 AND messages.recalled_at IS NULL \
                               AND (links.created_at < $2 OR (links.created_at = $2 AND \
                                 (links.message_id < $3 OR (links.message_id = $3 AND links.position > $4)))) \
                             ORDER BY links.created_at DESC, links.message_id DESC, links.position ASC \
                             LIMIT $5",
                        )
                        .bind(room_id)
                        .bind(created_at)
                        .bind(id)
                        .bind(position)
                        .bind(limit)
                        .fetch_all(pool)
                        .await
                    }
                    None => {
                        sqlx::query_as(
                            "SELECT links.message_id, links.position, links.url, messages.sender_id, \
                             messages.sender, links.created_at FROM message_links AS links \
                             JOIN messages ON messages.id = links.message_id \
                             WHERE links.room_id = $1 AND messages.recalled_at IS NULL \
                             ORDER BY links.created_at DESC, links.message_id DESC, links.position ASC \
                             LIMIT $2",
                        )
                        .bind(room_id)
                        .bind(limit)
                        .fetch_all(pool)
                        .await
                    }
                }
        })
    }
}
