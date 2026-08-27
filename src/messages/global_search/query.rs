use chrono::{DateTime, Utc};
use sqlx::FromRow;
use uuid::Uuid;

use super::models::{
    GlobalMessageSearchPage, GlobalMessageSearchResult, SearchContentType, SearchCursor,
    VisibleMessageSearch,
};
use crate::{
    messages::search_pattern::like_pattern,
    state::{with_pool, AppState},
};

#[derive(FromRow)]
struct SearchRow {
    message_id: Uuid,
    room_id: Uuid,
    conversation_kind: String,
    conversation_title: String,
    sender_id: Option<Uuid>,
    sender: String,
    content: String,
    attachment_file_name: Option<String>,
    attachment_mime_type: Option<String>,
    context_before: Option<String>,
    context_after: Option<String>,
    created_at: DateTime<Utc>,
}

impl SearchRow {
    fn cursor(&self) -> SearchCursor {
        SearchCursor {
            created_at: self.created_at,
            message_id: self.message_id,
        }
    }

    fn into_result(self) -> GlobalMessageSearchResult {
        GlobalMessageSearchResult {
            message_id: self.message_id,
            room_id: self.room_id,
            conversation_kind: self.conversation_kind,
            conversation_title: self.conversation_title,
            sender_id: self.sender_id,
            sender: self.sender,
            excerpt: excerpt(&self.content, 180),
            content_type: SearchContentType::from_mime_type(self.attachment_mime_type.as_deref()),
            attachment_file_name: self.attachment_file_name,
            context_before: self.context_before.map(|value| excerpt(&value, 100)),
            context_after: self.context_after.map(|value| excerpt(&value, 100)),
            created_at: self.created_at,
        }
    }
}

impl AppState {
    pub async fn search_visible_messages(
        &self,
        viewer_id: Uuid,
        search: &VisibleMessageSearch,
    ) -> Result<GlobalMessageSearchPage, sqlx::Error> {
        let pattern = like_pattern(&search.text);
        let content_type = search.content_type.as_str();
        let cursor_at = search.cursor.as_ref().map(|cursor| cursor.created_at);
        let cursor_id = search.cursor.as_ref().map(|cursor| cursor.message_id);
        let page_size = search.limit + 1;
        let mut rows: Vec<SearchRow> = with_pool!(self, |pool| {
            sqlx::query_as(
                "SELECT messages.id AS message_id, messages.room_id, \
                 CASE WHEN direct.room_id IS NULL THEN 'group' ELSE 'direct' END AS conversation_kind, \
                 COALESCE(NULLIF(memberships.conversation_alias, ''), \
                   CASE WHEN direct.room_id IS NULL THEN rooms.name \
                     ELSE COALESCE(NULLIF(remarks.remark, ''), NULLIF(peer.display_name, ''), peer.username) END \
                 ) AS conversation_title, messages.sender_id, messages.sender, messages.content, \
                 attachments.file_name AS attachment_file_name, \
                 attachments.mime_type AS attachment_mime_type, \
                 (SELECT previous.content FROM messages AS previous \
                   WHERE previous.room_id = messages.room_id AND previous.recalled_at IS NULL \
                     AND (previous.created_at < messages.created_at OR \
                       (previous.created_at = messages.created_at AND previous.id < messages.id)) \
                   ORDER BY previous.created_at DESC, previous.id DESC LIMIT 1) AS context_before, \
                 (SELECT following.content FROM messages AS following \
                   WHERE following.room_id = messages.room_id AND following.recalled_at IS NULL \
                     AND (following.created_at > messages.created_at OR \
                       (following.created_at = messages.created_at AND following.id > messages.id)) \
                   ORDER BY following.created_at ASC, following.id ASC LIMIT 1) AS context_after, \
                 messages.created_at \
                 FROM room_memberships AS memberships \
                 JOIN rooms ON rooms.id = memberships.room_id AND rooms.deleted_at IS NULL \
                 JOIN messages ON messages.room_id = rooms.id AND messages.recalled_at IS NULL \
                 LEFT JOIN attachments ON attachments.id = messages.attachment_id \
                 LEFT JOIN direct_conversations AS direct ON direct.room_id = rooms.id \
                 LEFT JOIN users AS peer ON peer.id = CASE \
                   WHEN direct.user_low_id = $1 THEN direct.user_high_id \
                   WHEN direct.user_high_id = $1 THEN direct.user_low_id ELSE NULL END \
                 LEFT JOIN friend_remarks AS remarks ON remarks.owner_id = $1 AND remarks.friend_id = peer.id \
                 WHERE memberships.user_id = $1 AND memberships.status = 'active' \
                   AND LOWER(messages.content) LIKE LOWER($2) ESCAPE '\\' \
                   AND ($3 IS NULL OR messages.room_id = $3) \
                   AND ($4 IS NULL OR messages.sender_id = $4) \
                   AND ($5 IS NULL OR messages.created_at >= $5) \
                   AND ($6 IS NULL OR messages.created_at <= $6) \
                   AND ($7 = 'all' OR ($7 = 'text' AND attachments.id IS NULL) \
                     OR ($7 = 'file' AND attachments.id IS NOT NULL) \
                     OR ($7 = 'image' AND attachments.mime_type LIKE 'image/%') \
                     OR ($7 = 'video' AND attachments.mime_type LIKE 'video/%') \
                     OR ($7 = 'audio' AND attachments.mime_type LIKE 'audio/%')) \
                   AND ($8 IS NULL OR messages.created_at < $8 OR \
                     (messages.created_at = $8 AND messages.id < $9)) \
                 ORDER BY messages.created_at DESC, messages.id DESC LIMIT $10",
            )
            .bind(viewer_id)
            .bind(&pattern)
            .bind(search.room_id)
            .bind(search.sender_id)
            .bind(search.from)
            .bind(search.to)
            .bind(content_type)
            .bind(cursor_at)
            .bind(cursor_id)
            .bind(page_size)
            .fetch_all(pool)
            .await
        })?;
        let has_more = rows.len() > search.limit as usize;
        rows.truncate(search.limit as usize);
        let next_cursor = if has_more {
            rows.last().map(SearchRow::cursor)
        } else {
            None
        };
        Ok(GlobalMessageSearchPage {
            items: rows.into_iter().map(SearchRow::into_result).collect(),
            next_cursor: next_cursor.map(|cursor| cursor.to_string()),
        })
    }
}

fn excerpt(value: &str, max_chars: usize) -> String {
    let value = value.trim();
    let mut excerpt = value.chars().take(max_chars).collect::<String>();
    if value.chars().count() > max_chars {
        excerpt.push_str("...");
    }
    excerpt
}

#[cfg(test)]
mod tests {
    use super::excerpt;

    #[test]
    fn excerpt_is_trimmed_and_bounded_by_characters() {
        assert_eq!(excerpt("  hello  ", 5), "hello");
        assert_eq!(excerpt("你好世界", 2), "你好...");
    }
}
