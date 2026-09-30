//! Channel post views and the batched `message_views_updated` frame (TG-202).
//!
//! A view is counted once per account and post: `message_views` holds one row per pair and
//! `messages.views_count` is its projection. Views are never written or broadcast one by one.
//! A reported view joins its chat's pending set; the first view of a quiet chat opens a
//! [`VIEW_BROADCAST_WINDOW`], and at its end one flush writes every pending pair in one
//! transaction (`INSERT … ON CONFLICT DO NOTHING RETURNING` tells which pairs are new), bumps
//! each post's counter by the new pairs, and sends **one** frame listing every post that
//! changed. So:
//!
//! - 1000 subscribers viewing the same posts cost one transaction and one frame per window —
//!   at most `t / window + 1` frames over `t` seconds, whatever the number of viewers;
//! - every counted view is reflected by a frame that starts after it (a view reported after
//!   a flush took the pending set opens a new window).
//!
//! The pending set is process-wide and keyed by chat id (the TG-406 poll-broadcast pattern,
//! which keeps this out of the `state*.rs` hotspot). A crash loses at most one window of
//! views; the counter and `message_views` never disagree because one transaction writes both.

use std::collections::{HashMap, HashSet};
use std::sync::{LazyLock, Mutex};
use std::time::Duration;

use chrono::Utc;
use sqlx::{FromRow, QueryBuilder};
use uuid::Uuid;

use super::ChatType;
use crate::models::{ChatMessage, MessageViewCount};
use crate::state::{with_pool, AppState, SharedState};

/// Coalescing window per chat.
pub const VIEW_BROADCAST_WINDOW: Duration = Duration::from_millis(500);

/// Posts one report may carry: a screenful, with room to spare.
pub const MAX_VIEWED_POSTS: usize = 100;

/// Pairs per multi-row insert: three binds each, far below either adapter's bind limit.
const INSERT_CHUNK: usize = 300;

type ViewPair = (Uuid, Uuid);

static PENDING: LazyLock<Mutex<HashMap<Uuid, HashSet<ViewPair>>>> = LazyLock::new(Default::default);

fn pending() -> std::sync::MutexGuard<'static, HashMap<Uuid, HashSet<ViewPair>>> {
    // Insert/remove only: a panic while holding the lock cannot leave it inconsistent.
    PENDING
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[derive(Debug)]
pub enum ViewError {
    /// No such channel, or the viewer does not read it.
    NotFound,
    /// Empty, or more than [`MAX_VIEWED_POSTS`] ids.
    Invalid,
    Database(sqlx::Error),
}

impl From<sqlx::Error> for ViewError {
    fn from(error: sqlx::Error) -> Self {
        ViewError::Database(error)
    }
}

#[derive(FromRow)]
struct CountRow {
    id: Uuid,
    views_count: i64,
}

/// Record that `viewer` saw `message_ids` in channel `room_id`. Returns the posts' counts as
/// stored now; this view reaches them — and every subscriber — with the next batched frame.
/// Ids that are not live posts of this channel are ignored.
pub async fn record_channel_views(
    state: &SharedState,
    room_id: Uuid,
    viewer: Uuid,
    message_ids: &[Uuid],
) -> Result<Vec<MessageViewCount>, ViewError> {
    if message_ids.is_empty() || message_ids.len() > MAX_VIEWED_POSTS {
        return Err(ViewError::Invalid);
    }
    if state.chat_type(room_id).await? != Some(ChatType::Channel)
        || !state.can_read_chat(room_id, viewer).await?
    {
        return Err(ViewError::NotFound);
    }
    let counts = state.live_post_counts(room_id, message_ids).await?;
    if counts.is_empty() {
        return Ok(counts);
    }
    let opens_window = {
        let mut pending = pending();
        let opens_window = !pending.contains_key(&room_id);
        let pairs = pending.entry(room_id).or_default();
        for count in &counts {
            pairs.insert((count.message_id, viewer));
        }
        opens_window
    };
    if opens_window {
        let state = state.clone();
        tokio::spawn(async move {
            tokio::time::sleep(VIEW_BROADCAST_WINDOW).await;
            flush_channel_views(&state, room_id).await;
        });
    }
    Ok(counts)
}

async fn flush_channel_views(state: &SharedState, room_id: Uuid) {
    let Some(pairs) = pending().remove(&room_id) else {
        return;
    };
    match state.apply_channel_views(pairs.into_iter().collect()).await {
        Ok(views) if !views.is_empty() => {
            state
                .broadcast(room_id, ChatMessage::MessageViewsUpdated { views })
                .await;
        }
        Ok(_) => {}
        Err(error) => tracing::warn!(%room_id, "apply channel views failed: {error}"),
    }
}

impl AppState {
    /// Current counts of the live (not recalled) posts of `room_id` among `ids`.
    async fn live_post_counts(
        &self,
        room_id: Uuid,
        ids: &[Uuid],
    ) -> Result<Vec<MessageViewCount>, sqlx::Error> {
        let rows: Vec<CountRow> = with_pool!(self, |pool| {
            let mut query = QueryBuilder::new(
                "SELECT id, CAST(views_count AS BIGINT) AS views_count FROM messages \
                 WHERE recalled_at IS NULL AND room_id = ",
            );
            query.push_bind(room_id);
            query.push(" AND id IN (");
            {
                let mut values = query.separated(", ");
                for id in ids {
                    values.push_bind(*id);
                }
            }
            query.push(")");
            query.build_query_as().fetch_all(pool).await
        })?;
        Ok(rows.into_iter().map(CountRow::into_count).collect())
    }

    /// One transaction: store the new (post, viewer) pairs, add them to the counters, and
    /// return the new counts of every post that gained a view.
    async fn apply_channel_views(
        &self,
        pairs: Vec<ViewPair>,
    ) -> Result<Vec<MessageViewCount>, sqlx::Error> {
        let now = Utc::now();
        with_pool!(self, |pool| {
            async {
                let mut transaction = pool.begin().await?;
                let mut gained: HashMap<Uuid, i64> = HashMap::new();
                for chunk in pairs.chunks(INSERT_CHUNK) {
                    let mut insert = QueryBuilder::new(
                        "INSERT INTO message_views (message_id, user_id, viewed_at) ",
                    );
                    insert.push_values(chunk, |mut row, (message_id, user_id)| {
                        row.push_bind(*message_id)
                            .push_bind(*user_id)
                            .push_bind(now);
                    });
                    insert
                        .push(" ON CONFLICT (message_id, user_id) DO NOTHING RETURNING message_id");
                    let inserted: Vec<Uuid> = insert
                        .build_query_scalar()
                        .fetch_all(&mut *transaction)
                        .await?;
                    for message_id in inserted {
                        *gained.entry(message_id).or_default() += 1;
                    }
                }
                let mut views = Vec::with_capacity(gained.len());
                for (message_id, added) in gained {
                    let total: i64 = sqlx::query_scalar(
                        "UPDATE messages SET views_count = views_count + $1 WHERE id = $2 \
                         RETURNING CAST(views_count AS BIGINT)",
                    )
                    .bind(added)
                    .bind(message_id)
                    .fetch_one(&mut *transaction)
                    .await?;
                    views.push(MessageViewCount {
                        message_id,
                        views: total,
                    });
                }
                transaction.commit().await?;
                views.sort_by_key(|view| view.message_id);
                Ok::<_, sqlx::Error>(views)
            }
            .await
        })
    }
}

impl CountRow {
    fn into_count(self) -> MessageViewCount {
        MessageViewCount {
            message_id: self.id,
            views: self.views_count,
        }
    }
}
