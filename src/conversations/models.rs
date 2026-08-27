use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize};
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum NotificationLevel {
    All,
    Mentions,
    None,
}

impl NotificationLevel {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Mentions => "mentions",
            Self::None => "none",
        }
    }

    pub(crate) fn from_database(value: &str) -> Self {
        match value {
            "mentions" => Self::Mentions,
            "none" => Self::None,
            _ => Self::All,
        }
    }
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ConversationPreferences {
    pub room_id: Uuid,
    pub is_pinned: bool,
    pub is_archived: bool,
    pub notification_level: NotificationLevel,
    pub muted_until: Option<DateTime<Utc>>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Default, Deserialize, ToSchema)]
pub struct UpdateConversationPreferencesRequest {
    pub is_pinned: Option<bool>,
    pub is_archived: Option<bool>,
    pub notification_level: Option<NotificationLevel>,
    #[serde(default, deserialize_with = "deserialize_optional_field")]
    #[schema(value_type = Option<String>)]
    pub muted_until: Option<Option<DateTime<Utc>>>,
}

impl UpdateConversationPreferencesRequest {
    pub(crate) fn is_empty(&self) -> bool {
        self.is_pinned.is_none()
            && self.is_archived.is_none()
            && self.notification_level.is_none()
            && self.muted_until.is_none()
    }
}

fn deserialize_optional_field<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer).map(Some)
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
    pub preferences: ConversationPreferences,
    pub last_message: Option<MessagePreview>,
    pub last_activity_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateConversationAliasRequest {
    pub alias: String,
}
