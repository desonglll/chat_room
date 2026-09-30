//! Assembling what one viewer sees of a forum's topics, and announcing topic changes.

use chrono::{DateTime, Utc};
use uuid::Uuid;

use super::model::{ForumTopic, TopicLastMessage};
use super::reads::TopicMute;
use super::store::TopicRow;
use crate::models::ChatMessage;
use crate::realtime::payloads::TopicSummary;
use crate::state::{with_pool, AppState};

type LastMessageRow = (Uuid, String, String, Option<DateTime<Utc>>, DateTime<Utc>);

impl AppState {
    async fn topic_last_message(
        &self,
        topic: &TopicRow,
    ) -> Result<Option<TopicLastMessage>, sqlx::Error> {
        let query = if topic.is_general {
            "SELECT id, sender, content, recalled_at, created_at FROM messages \
             WHERE room_id = $1 AND topic_id IS NULL AND CAST($2 AS TEXT) IS NULL \
             ORDER BY created_at DESC, id DESC LIMIT 1"
        } else {
            "SELECT id, sender, content, recalled_at, created_at FROM messages \
             WHERE room_id = $1 AND topic_id = $2 ORDER BY created_at DESC, id DESC LIMIT 1"
        };
        let row: Option<LastMessageRow> = with_pool!(self, |pool| {
            sqlx::query_as(query)
                .bind(topic.room_id)
                .bind(topic.stored_id())
                .fetch_optional(pool)
                .await
        })?;
        Ok(row.map(
            |(message_id, sender, content, recalled_at, created_at)| TopicLastMessage {
                message_id,
                sender,
                content: if recalled_at.is_some() {
                    String::new()
                } else {
                    content
                },
                created_at,
            },
        ))
    }

    /// Every topic as `viewer_id` sees it, in list order. `is_admin` is the viewer's topic
    /// administrator status (it decides `can_edit`).
    pub(crate) async fn topic_views(
        &self,
        room_id: Uuid,
        viewer_id: Uuid,
        is_admin: bool,
        rows: Vec<TopicRow>,
    ) -> Result<Vec<ForumTopic>, sqlx::Error> {
        let Some(general_id) = rows.iter().find(|row| row.is_general).map(|row| row.id) else {
            return Ok(Vec::new());
        };
        let unread = self
            .topic_unread_counts(room_id, viewer_id, general_id)
            .await?;
        let mutes = self.topic_mutes(room_id, viewer_id).await?;
        let now = Utc::now();
        let mut views = Vec::with_capacity(rows.len());
        for row in rows {
            let last_message = self.topic_last_message(&row).await?;
            let mute = mutes.get(&row.id).copied().unwrap_or_default();
            views.push(topic_view(
                row,
                last_message,
                &unread,
                mute,
                now,
                is_admin,
                viewer_id,
            ));
        }
        views.sort_by_key(ForumTopic::sort_key);
        Ok(views)
    }

    pub(crate) async fn single_topic_view(
        &self,
        row: TopicRow,
        viewer_id: Uuid,
        is_admin: bool,
    ) -> Result<ForumTopic, sqlx::Error> {
        let room_id = row.room_id;
        let id = row.id;
        let general = self.ensure_general_topic(room_id).await?;
        let mut rows = vec![row];
        if general.id != id {
            rows.push(general);
        }
        let views = self.topic_views(room_id, viewer_id, is_admin, rows).await?;
        Ok(views
            .into_iter()
            .find(|view| view.id == id)
            .expect("the requested topic is among the views"))
    }

    pub(crate) async fn announce_topic(&self, room_id: Uuid, topic: TopicSummary) {
        self.broadcast(room_id, ChatMessage::TopicUpdated { topic })
            .await;
    }
}

fn topic_view(
    row: TopicRow,
    last_message: Option<TopicLastMessage>,
    unread: &std::collections::HashMap<Option<Uuid>, i64>,
    mute: TopicMute,
    now: DateTime<Utc>,
    is_admin: bool,
    viewer_id: Uuid,
) -> ForumTopic {
    let unread_count = unread.get(&row.stored_id()).copied().unwrap_or(0);
    let muted = mute.active(now);
    ForumTopic {
        id: row.id,
        chat_id: row.room_id,
        is_general: row.is_general,
        can_edit: is_admin || (!row.is_general && row.creator_id == Some(viewer_id)),
        title: row.title,
        icon_emoji: row.icon_emoji,
        icon_custom_emoji_id: row.icon_custom_emoji_id,
        icon_color: row.icon_color,
        is_pinned: row.pinned_at.is_some(),
        pinned_at: row.pinned_at,
        is_closed: row.closed_at.is_some(),
        is_hidden: row.is_hidden,
        creator_id: row.creator_id,
        created_at: row.created_at,
        last_message,
        unread_count,
        muted,
        muted_until: if muted { mute.muted_until } else { None },
    }
}
