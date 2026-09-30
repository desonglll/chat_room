use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::chats::ChatType;
use crate::conversations::models::{
    ConversationPreferences, ConversationSummary, MessagePreview, NotificationLevel,
};
use crate::models::{Chat, ChatCompatView, UserSummary};
use crate::state::{with_pool, AppState};

#[derive(sqlx::FromRow)]
struct ConversationRow {
    room_id: Uuid,
    kind: String,
    #[sqlx(try_from = "String")]
    chat_type: ChatType,
    title: String,
    conversation_alias: String,
    display_avatar: String,
    display_description: String,
    room_name: String,
    chat_username: Option<String>,
    is_forum: bool,
    member_count: i64,
    has_password: bool,
    creator_user_id: Option<Uuid>,
    join_policy: String,
    chat_avatar: String,
    chat_description: String,
    membership_status: String,
    membership_role: String,
    unread_count: i64,
    pending_join_requests: i64,
    pending_join_requested_at: Option<DateTime<Utc>>,
    is_pinned: bool,
    is_archived: bool,
    notification_level: String,
    muted_until: Option<DateTime<Utc>>,
    preferences_updated_at: DateTime<Utc>,
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
        // The sidebar's shape depends on chat_type (docs/tg/architecture.md §4.3), so the
        // conversation list carries it rather than making the client fetch each chat. `kind`
        // is the frozen clients' spelling of the same fact: 'direct' iff chat_type = 'private'.
        let group = (self.kind == "group").then(|| {
            ChatCompatView::from(Chat {
                id: self.room_id,
                chat_type: self.chat_type,
                title: self.room_name,
                has_password: self.has_password,
                creator_user_id: self.creator_user_id,
                join_policy: self.join_policy,
                avatar_emoji: self.chat_avatar,
                description: self.chat_description,
                username: self.chat_username,
                is_forum: self.is_forum,
                member_count: self.member_count,
                membership_status: Some(self.membership_status),
                membership_role: Some(self.membership_role),
                unread_count: self.unread_count,
                created_at: self.created_at,
                ..Chat::default()
            })
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
            chat_type: self.chat_type,
            kind: self.kind,
            title: self.title,
            alias: self.conversation_alias,
            avatar_emoji: self.display_avatar,
            description: self.display_description,
            group,
            peer,
            unread_count: self.unread_count,
            pending_join_requests: self.pending_join_requests,
            preferences: ConversationPreferences {
                room_id: self.room_id,
                is_pinned: self.is_pinned,
                is_archived: self.is_archived,
                notification_level: NotificationLevel::from_database(&self.notification_level),
                muted_until: self.muted_until,
                updated_at: self.preferences_updated_at,
            },
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
                "SELECT chats.id AS room_id, \
                 CASE WHEN chats.chat_type = 'private' THEN 'direct' ELSE 'group' END AS kind, \
                 CASE WHEN chats.chat_type <> 'private' THEN chats.title \
                   ELSE COALESCE(NULLIF(remarks.remark, ''), NULLIF(peer.display_name, ''), peer.username) END AS title, \
                 memberships.conversation_alias, \
                 CASE WHEN chats.chat_type <> 'private' THEN chats.avatar_emoji \
                   ELSE COALESCE(peer.avatar_emoji, '') END AS display_avatar, \
                 CASE WHEN chats.chat_type <> 'private' THEN chats.description \
                   ELSE COALESCE(peer.signature, '') END AS display_description, \
                 chats.title AS room_name, chats.chat_type, chats.username AS chat_username, \
                 chats.is_forum, CAST(chats.member_count AS BIGINT) AS member_count, \
                 chats.password_hash <> '' AS has_password, \
                 chats.creator_user_id, chats.join_policy, chats.avatar_emoji AS chat_avatar, \
                 chats.description AS chat_description, memberships.status AS membership_status, \
                 roles.name AS membership_role, \
                 CAST((SELECT COUNT(unread.id) FROM messages AS unread \
                   LEFT JOIN chat_reads AS reads ON reads.room_id = chats.id AND reads.user_id = $1 \
                   LEFT JOIN messages AS read_message ON read_message.id = reads.message_id \
                   WHERE unread.room_id = chats.id AND unread.recalled_at IS NULL \
                     AND (unread.sender_id IS NULL OR unread.sender_id <> $1) \
                     AND (read_message.id IS NULL OR unread.created_at > read_message.created_at \
                       OR (unread.created_at = read_message.created_at AND unread.id > read_message.id))) \
                   AS BIGINT) AS unread_count, chats.created_at, \
                 CAST(CASE WHEN review.role_id IS NULL THEN 0 \
                   ELSE (SELECT COUNT(*) FROM chat_members AS requests \
                     WHERE requests.room_id = chats.id AND requests.status = 'pending') \
                   END AS BIGINT) AS pending_join_requests, \
                 CASE WHEN review.role_id IS NULL THEN NULL ELSE \
                   (SELECT MAX(requests.requested_at) FROM chat_members AS requests \
                     WHERE requests.room_id = chats.id AND requests.status = 'pending') \
                   END AS pending_join_requested_at, \
                 memberships.is_pinned, memberships.is_archived, \
                 memberships.notification_level, memberships.muted_until, \
                 memberships.preferences_updated_at, chats.created_at, \
                 COALESCE(last_message.created_at, chats.created_at) AS last_activity_at, \
                 peer.id AS peer_id, peer.username AS peer_username, \
                 peer.avatar_emoji AS peer_avatar, peer.display_name AS peer_display_name, \
                 last_message.id AS last_message_id, last_message.sender_id AS last_sender_id, \
                 last_message.sender AS last_sender, last_message.content AS last_content, \
                 attachments.file_name AS last_attachment_file_name, \
                 last_message.recalled_at AS last_recalled_at, \
                 last_message.created_at AS last_created_at \
                 FROM chat_members AS memberships \
                 JOIN chats ON chats.id = memberships.room_id AND chats.deleted_at IS NULL \
                 JOIN chat_roles AS roles ON roles.id = memberships.role_id \
                 LEFT JOIN chat_role_permissions AS review ON review.role_id = roles.id \
                   AND review.permission_key = 'members.review' \
                 LEFT JOIN direct_conversations AS direct ON direct.room_id = chats.id \
                   AND chats.chat_type = 'private' \
                 LEFT JOIN users AS peer ON peer.id = CASE \
                   WHEN direct.user_low_id = $1 THEN direct.user_high_id \
                   WHEN direct.user_high_id = $1 THEN direct.user_low_id ELSE NULL END \
                 LEFT JOIN friend_remarks AS remarks ON remarks.owner_id = $1 \
                   AND remarks.friend_id = peer.id \
                 LEFT JOIN messages AS last_message ON last_message.id = ( \
                   SELECT candidate.id FROM messages AS candidate \
                   WHERE candidate.room_id = chats.id \
                   ORDER BY candidate.created_at DESC, candidate.id DESC LIMIT 1) \
                 LEFT JOIN attachments ON attachments.id = last_message.attachment_id \
                 WHERE memberships.user_id = $1 AND memberships.status = 'active' \
                   AND ($2 IS NULL OR chats.id = $2) \
                 ORDER BY memberships.is_archived ASC, \
                   CASE WHEN memberships.is_archived THEN FALSE ELSE memberships.is_pinned END DESC, \
                   last_activity_at DESC, chats.id",
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
                "UPDATE chat_members SET conversation_alias = $1 \
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
