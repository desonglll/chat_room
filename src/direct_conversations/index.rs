//! Reads and writes of the `direct_conversations` index table, and nothing else.
//!
//! The two statement functions take any executor so that `chats::private_chats` can run them
//! inside the same transaction that provisions the chat itself.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use sqlx::{ColumnIndex, Database, Decode, Encode, Executor, IntoArguments, Type};
use uuid::Uuid;

use crate::state::{with_pool, AppState};

/// The live (not soft-deleted) chat that the canonical pair `low < high` already shares.
pub(crate) async fn chat_for_pair<'c, E, DB>(
    executor: E,
    low: Uuid,
    high: Uuid,
) -> Result<Option<Uuid>, sqlx::Error>
where
    E: Executor<'c, Database = DB>,
    DB: Database,
    for<'q> DB::Arguments<'q>: IntoArguments<'q, DB>,
    for<'q> Uuid: Encode<'q, DB> + Decode<'q, DB> + Type<DB>,
    usize: ColumnIndex<DB::Row>,
{
    sqlx::query_scalar(
        "SELECT direct_conversations.room_id FROM direct_conversations \
         JOIN chats ON chats.id = direct_conversations.room_id \
         WHERE user_low_id = $1 AND user_high_id = $2 AND chats.deleted_at IS NULL",
    )
    .bind(low)
    .bind(high)
    .fetch_optional(executor)
    .await
}

/// Record that `chat_id` is the private chat of the canonical pair `low < high`.
pub(crate) async fn record_pair<'c, E, DB>(
    executor: E,
    chat_id: Uuid,
    low: Uuid,
    high: Uuid,
    created_at: DateTime<Utc>,
) -> Result<(), sqlx::Error>
where
    E: Executor<'c, Database = DB>,
    DB: Database,
    for<'q> DB::Arguments<'q>: IntoArguments<'q, DB>,
    for<'q> Uuid: Encode<'q, DB> + Type<DB>,
    for<'q> DateTime<Utc>: Encode<'q, DB> + Type<DB>,
{
    sqlx::query(
        "INSERT INTO direct_conversations (room_id, user_low_id, user_high_id, created_at) \
         VALUES ($1, $2, $3, $4)",
    )
    .bind(chat_id)
    .bind(low)
    .bind(high)
    .bind(created_at)
    .execute(executor)
    .await
    .map(|_| ())
}

/// The other participant of one private chat, as the viewer sees them.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PrivateChatPeer {
    pub chat_id: Uuid,
    pub peer_id: Uuid,
    pub username: String,
    pub display_name: String,
    pub avatar_emoji: String,
    pub signature: String,
    /// The viewer's own remark for the peer; empty when none is set.
    pub remark: String,
}

impl AppState {
    /// Every private chat `viewer_id` is indexed in, with the other participant's profile.
    ///
    /// Soft-deleted chats are included: the caller already holds the chats it wants to
    /// present and looks them up in the returned map.
    pub async fn private_chat_peers(
        &self,
        viewer_id: Uuid,
    ) -> Result<HashMap<Uuid, PrivateChatPeer>, sqlx::Error> {
        let rows: Vec<PrivateChatPeer> = with_pool!(self, |pool| {
            sqlx::query_as(
                "SELECT direct.room_id AS chat_id, peer.id AS peer_id, peer.username, \
                   peer.display_name, peer.avatar_emoji, peer.signature, \
                   COALESCE(remarks.remark, '') AS remark \
                 FROM direct_conversations AS direct \
                 JOIN users AS peer ON peer.id = CASE WHEN direct.user_low_id = $1 \
                   THEN direct.user_high_id ELSE direct.user_low_id END \
                 LEFT JOIN friend_remarks AS remarks ON remarks.owner_id = $1 \
                   AND remarks.friend_id = peer.id \
                 WHERE direct.user_low_id = $1 OR direct.user_high_id = $1",
            )
            .bind(viewer_id)
            .fetch_all(pool)
            .await
        })?;
        Ok(rows.into_iter().map(|row| (row.chat_id, row)).collect())
    }
}
