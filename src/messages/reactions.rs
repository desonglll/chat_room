//! Persistent, room-scoped emoji responses for chat messages.

use std::collections::HashMap;

use chrono::Utc;
use sqlx::{FromRow, Postgres, QueryBuilder, Sqlite};
use uuid::Uuid;

use crate::models::{MessageReaction, StoredMessage};
use crate::state::{with_pool, AppState};

pub const ALLOWED_REACTIONS: [&str; 6] = ["👍", "❤️", "😂", "😮", "😢", "👏"];

#[derive(FromRow)]
struct ReactionRow {
    message_id: Uuid,
    emoji: String,
    user_id: Uuid,
}

impl AppState {
    pub(crate) async fn set_message_reaction(
        &self,
        room_id: Uuid,
        user_id: Uuid,
        message_id: Uuid,
        emoji: &str,
        active: bool,
    ) -> Result<bool, sqlx::Error> {
        if !ALLOWED_REACTIONS.contains(&emoji) {
            return Ok(false);
        }
        let allowed: bool = with_pool!(self, |pool| {
            sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM messages \
                 JOIN room_memberships ON room_memberships.room_id = messages.room_id \
                   AND room_memberships.user_id = $3 AND room_memberships.status = 'active' \
                 JOIN room_role_permissions ON room_role_permissions.role_id = room_memberships.role_id \
                   AND room_role_permissions.permission_key = 'message.send' \
                 WHERE messages.id = $1 AND messages.room_id = $2 AND messages.recalled_at IS NULL)",
            )
            .bind(message_id)
            .bind(room_id)
            .bind(user_id)
            .fetch_one(pool)
            .await
        })?;
        if !allowed {
            return Ok(false);
        }

        if active {
            with_pool!(self, |pool| {
                sqlx::query(
                    "INSERT INTO message_reactions (message_id, user_id, emoji, created_at) \
                     VALUES ($1, $2, $3, $4) ON CONFLICT (message_id, user_id, emoji) DO NOTHING",
                )
                .bind(message_id)
                .bind(user_id)
                .bind(emoji)
                .bind(Utc::now())
                .execute(pool)
                .await
                .map(|_| ())
            })?;
        } else {
            with_pool!(self, |pool| {
                sqlx::query(
                    "DELETE FROM message_reactions WHERE message_id = $1 AND user_id = $2 AND emoji = $3",
                )
                .bind(message_id)
                .bind(user_id)
                .bind(emoji)
                .execute(pool)
                .await
                .map(|_| ())
            })?;
        }
        Ok(true)
    }

    pub(crate) async fn attach_message_reactions(
        &self,
        messages: &mut [StoredMessage],
    ) -> Result<(), sqlx::Error> {
        if messages.is_empty() {
            return Ok(());
        }
        let ids: Vec<Uuid> = messages.iter().map(|message| message.id).collect();
        let rows = match self.database_pool() {
            crate::storage::DatabasePool::Sqlite(pool) => reaction_rows_sqlite(pool, &ids).await?,
            crate::storage::DatabasePool::Postgres(pool) => {
                reaction_rows_postgres(pool, &ids).await?
            }
        };
        let mut grouped: HashMap<Uuid, Vec<MessageReaction>> = HashMap::new();
        for row in rows {
            let reactions = grouped.entry(row.message_id).or_default();
            if let Some(reaction) = reactions
                .iter_mut()
                .find(|reaction| reaction.emoji == row.emoji)
            {
                reaction.user_ids.push(row.user_id);
            } else {
                reactions.push(MessageReaction {
                    emoji: row.emoji,
                    user_ids: vec![row.user_id],
                });
            }
        }
        for message in messages {
            message.reactions = grouped.remove(&message.id).unwrap_or_default();
        }
        Ok(())
    }
}

async fn reaction_rows_sqlite(
    pool: &sqlx::SqlitePool,
    ids: &[Uuid],
) -> Result<Vec<ReactionRow>, sqlx::Error> {
    let mut query = QueryBuilder::<Sqlite>::new(
        "SELECT message_id, emoji, user_id FROM message_reactions WHERE message_id IN (",
    );
    {
        let mut values = query.separated(", ");
        for id in ids {
            values.push_bind(*id);
        }
    }
    query.push(") ORDER BY created_at, emoji, user_id");
    query.build_query_as().fetch_all(pool).await
}

async fn reaction_rows_postgres(
    pool: &sqlx::PgPool,
    ids: &[Uuid],
) -> Result<Vec<ReactionRow>, sqlx::Error> {
    let mut query = QueryBuilder::<Postgres>::new(
        "SELECT message_id, emoji, user_id FROM message_reactions WHERE message_id IN (",
    );
    {
        let mut values = query.separated(", ");
        for id in ids {
            values.push_bind(*id);
        }
    }
    query.push(") ORDER BY created_at, emoji, user_id");
    query.build_query_as().fetch_all(pool).await
}
