use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::models::{Room, UserSummary};

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct MessagePreview {
    pub message_id: Uuid,
    pub sender_id: Option<Uuid>,
    pub sender: String,
    pub content: String,
    pub attachment_file_name: Option<String>,
    pub recalled: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ConversationSummary {
    pub room_id: Uuid,
    pub kind: String,
    pub title: String,
    pub alias: String,
    pub avatar_emoji: String,
    pub description: String,
    pub group: Option<Room>,
    pub peer: Option<UserSummary>,
    pub unread_count: i64,
    pub pending_join_requests: i64,
    pub last_message: Option<MessagePreview>,
    pub last_activity_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateConversationAliasRequest {
    pub alias: String,
}
