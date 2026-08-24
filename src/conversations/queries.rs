use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::conversations::models::{ConversationSummary, MessagePreview};
use crate::models::{Room, UserSummary};
use crate::state::{with_pool, AppState};

#[derive(sqlx::FromRow)]
struct ConversationRow {
    room_id: Uuid,
    kind: String,
    title: String,
    conversation_alias: String,
    display_avatar: String,
    display_description: String,
    room_name: String,
    has_password: bool,
    creator_user_id: Option<Uuid>,
    join_policy: String,
    room_avatar: String,
    room_description: String,
    membership_status: String,
    membership_role: String,
    unread_count: i64,
    pending_join_requests: i64,
    pending_join_requested_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    last_activity_at: DateTime<Utc>,
    peer_id: Option<Uuid>,
    peer_username: Option<String>,
    peer_avatar: Option<String>,
    peer_display_name: Option<String>,
    last_message_id: Option<Uuid>,
    last_sender_id: Option<Uuid>,
    last_sender: Option<String>,
    last_content: Option<String>,
    last_attachment_file_name: Option<String>,
    last_recalled_at: Option<DateTime<Utc>>,
    last_created_at: Option<DateTime<Utc>>,
}

fn preview_content(value: &str) -> String {
    value.chars().take(120).collect()
}

impl ConversationRow {
    fn into_summary(self) -> ConversationSummary {
        let last_activity_at = self
            .pending_join_requested_at
            .map_or(self.last_activity_at, |requested_at| {
                requested_at.max(self.last_activity_at)
            });
        let group = (self.kind == "group").then(|| Room {
            id: self.room_id,
            name: self.room_name,
            password_hash: String::new(),
            has_password: self.has_password,
            creator_user_id: self.creator_user_id,
            join_policy: self.join_policy,
            avatar_emoji: self.room_avatar,
            description: self.room_description,
            membership_status: Some(self.membership_status),
            membership_role: Some(self.membership_role),
            unread_count: self.unread_count,
            created_at: self.created_at,
        });
        let peer = self.peer_id.map(|id| UserSummary {
            id,
            username: self.peer_username.unwrap_or_default(),
            avatar_emoji: self.peer_avatar.unwrap_or_default(),
            display_name: self.peer_display_name.unwrap_or_default(),
        });
        let last_message = self.last_message_id.map(|message_id| MessagePreview {
            message_id,
            sender_id: self.last_sender_id,
            sender: self.last_sender.unwrap_or_default(),
            content: preview_content(&self.last_content.unwrap_or_default()),
            attachment_file_name: self.last_attachment_file_name,
            recalled: self.last_recalled_at.is_some(),
            created_at: self.last_created_at.unwrap_or(self.created_at),
        });
        ConversationSummary {
            room_id: self.room_id,
            kind: self.kind,
            title: self.title,
            alias: self.conversation_alias,
            avatar_emoji: self.display_avatar,
            description: self.display_description,
            group,
            peer,
            unread_count: self.unread_count,
            pending_join_requests: self.pending_join_requests,
            last_message,
            last_activity_at,
            created_at: self.created_at,
        }
    }
}

impl AppState {
    async fn conversation_rows(
        &self,
        user_id: Uuid,
        room_id: Option<Uuid>,
    ) -> Result<Vec<ConversationRow>, sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query_as(
                "SELECT rooms.id AS room_id, \
                 CASE WHEN direct.room_id IS NULL THEN 'group' ELSE 'direct' END AS kind, \
                 CASE WHEN direct.room_id IS NULL THEN rooms.name \
                   ELSE COALESCE(NULLIF(peer.display_name, ''), peer.username) END AS title, \
                 memberships.conversation_alias, \
                 CASE WHEN direct.room_id IS NULL THEN rooms.avatar_emoji \
                   ELSE COALESCE(peer.avatar_emoji, '') END AS display_avatar, \
                 CASE WHEN direct.room_id IS NULL THEN rooms.description \
                   ELSE COALESCE(peer.signature, '') END AS display_description, \
                 rooms.name AS room_name, rooms.password_hash <> '' AS has_password, \
                 rooms.creator_user_id, rooms.join_policy, rooms.avatar_emoji AS room_avatar, \
                 rooms.description AS room_description, memberships.status AS membership_status, \
                 roles.name AS membership_role, \
                 CAST((SELECT COUNT(unread.id) FROM messages AS unread \
                   LEFT JOIN room_reads AS reads ON reads.room_id = rooms.id AND reads.user_id = $1 \
                   LEFT JOIN messages AS read_message ON read_message.id = reads.message_id \
                   WHERE unread.room_id = rooms.id AND unread.recalled_at IS NULL \
                     AND (unread.sender_id IS NULL OR unread.sender_id <> $1) \
                     AND (read_message.id IS NULL OR unread.created_at > read_message.created_at \
                       OR (unread.created_at = read_message.created_at AND unread.id > read_message.id))) \
                   AS BIGINT) AS unread_count, rooms.created_at, \
                 CAST(CASE WHEN review.role_id IS NULL THEN 0 \
                   ELSE (SELECT COUNT(*) FROM room_memberships AS requests \
                     WHERE requests.room_id = rooms.id AND requests.status = 'pending') \
                   END AS BIGINT) AS pending_join_requests, \
                 CASE WHEN review.role_id IS NULL THEN NULL ELSE \
                   (SELECT MAX(requests.requested_at) FROM room_memberships AS requests \
                     WHERE requests.room_id = rooms.id AND requests.status = 'pending') \
                   END AS pending_join_requested_at, rooms.created_at, \
                 COALESCE(last_message.created_at, rooms.created_at) AS last_activity_at, \
                 peer.id AS peer_id, peer.username AS peer_username, \
                 peer.avatar_emoji AS peer_avatar, peer.display_name AS peer_display_name, \
                 last_message.id AS last_message_id, last_message.sender_id AS last_sender_id, \
                 last_message.sender AS last_sender, last_message.content AS last_content, \
                 attachments.file_name AS last_attachment_file_name, \
                 last_message.recalled_at AS last_recalled_at, \
                 last_message.created_at AS last_created_at \
                 FROM room_memberships AS memberships \
                 JOIN rooms ON rooms.id = memberships.room_id AND rooms.deleted_at IS NULL \
                 JOIN room_roles AS roles ON roles.id = memberships.role_id \
                 LEFT JOIN room_role_permissions AS review ON review.role_id = roles.id \
                   AND review.permission_key = 'members.review' \
                 LEFT JOIN direct_conversations AS direct ON direct.room_id = rooms.id \
                 LEFT JOIN users AS peer ON peer.id = CASE \
                   WHEN direct.user_low_id = $1 THEN direct.user_high_id \
                   WHEN direct.user_high_id = $1 THEN direct.user_low_id ELSE NULL END \
                 LEFT JOIN messages AS last_message ON last_message.id = ( \
                   SELECT candidate.id FROM messages AS candidate \
                   WHERE candidate.room_id = rooms.id \
                   ORDER BY candidate.created_at DESC, candidate.id DESC LIMIT 1) \
                 LEFT JOIN attachments ON attachments.id = last_message.attachment_id \
                 WHERE memberships.user_id = $1 AND memberships.status = 'active' \
                   AND ($2 IS NULL OR rooms.id = $2) \
                 ORDER BY last_activity_at DESC, rooms.id",
            )
            .bind(user_id)
            .bind(room_id)
            .fetch_all(pool)
            .await
        })
    }

    pub async fn conversation_summaries(
        &self,
        user_id: Uuid,
    ) -> Result<Vec<ConversationSummary>, sqlx::Error> {
        self.conversation_rows(user_id, None).await.map(|rows| {
            rows.into_iter()
                .map(ConversationRow::into_summary)
                .collect()
        })
    }

    pub async fn conversation_summary(
        &self,
        user_id: Uuid,
        room_id: Uuid,
    ) -> Result<Option<ConversationSummary>, sqlx::Error> {
        self.conversation_rows(user_id, Some(room_id))
            .await
            .map(|rows| rows.into_iter().next().map(ConversationRow::into_summary))
    }

    pub async fn set_conversation_alias(
        &self,
        user_id: Uuid,
        room_id: Uuid,
        alias: &str,
    ) -> Result<bool, sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query(
                "UPDATE room_memberships SET conversation_alias = $1 \
                 WHERE user_id = $2 AND room_id = $3 AND status = 'active'",
            )
            .bind(alias)
            .bind(user_id)
            .bind(room_id)
            .execute(pool)
            .await
            .map(|result| result.rows_affected() > 0)
        })
    }
}
