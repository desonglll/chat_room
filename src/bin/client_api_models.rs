//! Response and request models for the terminal client's released API surface.

use serde::Deserialize;
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize)]
pub struct AuthSession {
    pub token: Uuid,
    pub user: UserIdentity,
}

#[derive(Clone, Debug, Deserialize)]
pub struct UserIdentity {
    pub username: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct RoomSummary {
    pub id: Uuid,
    pub name: String,
    #[serde(default)]
    pub has_password: bool,
    #[serde(default)]
    pub membership_status: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct RoomMembership {
    pub status: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ConversationPreferences {
    #[serde(default)]
    pub is_pinned: bool,
    #[serde(default)]
    pub is_archived: bool,
    #[serde(default = "default_notification_level")]
    pub notification_level: String,
    #[serde(default)]
    pub muted_until: Option<String>,
}

impl Default for ConversationPreferences {
    fn default() -> Self {
        Self {
            is_pinned: false,
            is_archived: false,
            notification_level: default_notification_level(),
            muted_until: None,
        }
    }
}

fn default_notification_level() -> String {
    "all".into()
}

#[derive(Clone, Debug, Deserialize)]
pub struct ConversationGroup {
    #[serde(default)]
    pub has_password: bool,
}

#[derive(Clone, Debug, Deserialize)]
pub struct MessagePreview {
    pub sender: String,
    pub content: String,
    #[serde(default)]
    pub recalled: bool,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Conversation {
    pub room_id: Uuid,
    pub kind: String,
    pub title: String,
    #[serde(default)]
    pub unread_count: i64,
    #[serde(default)]
    pub group: Option<ConversationGroup>,
    #[serde(default)]
    pub preferences: ConversationPreferences,
    #[serde(default)]
    pub last_message: Option<MessagePreview>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct SearchResult {
    pub message_id: Uuid,
    pub room_id: Uuid,
    pub conversation_title: String,
    pub sender: String,
    pub excerpt: String,
    #[serde(default)]
    pub content_type: String,
    #[serde(default)]
    pub attachment_file_name: Option<String>,
    pub created_at: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct SearchPage {
    pub items: Vec<SearchResult>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Notification {
    pub id: String,
    pub kind: String,
    pub summary: String,
    #[serde(default)]
    pub room_id: Option<Uuid>,
    #[serde(default)]
    pub room_name: Option<String>,
    #[serde(default)]
    pub message_id: Option<Uuid>,
    #[serde(default)]
    pub source_available: bool,
    #[serde(default)]
    pub read_at: Option<String>,
    pub created_at: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct NotificationPage {
    pub items: Vec<Notification>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Favorite {
    pub id: Uuid,
    pub title: String,
    pub content: String,
    pub kind: String,
    pub access: String,
    pub version: i64,
    #[serde(default)]
    pub source_room_id: Option<Uuid>,
    #[serde(default)]
    pub source_message_id: Option<Uuid>,
    #[serde(default)]
    pub source_room_name: String,
    #[serde(default)]
    pub source_sender: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct AiThread {
    pub id: Uuid,
    pub title: String,
    #[serde(default)]
    pub room_id: Option<Uuid>,
    pub updated_at: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct AiThreadMessage {
    pub id: Uuid,
    pub role: String,
    pub content: String,
    pub status: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct AiRun {
    pub id: Uuid,
    pub status: String,
    #[serde(default)]
    pub error_message: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct PreferencePatch {
    pub is_pinned: Option<bool>,
    pub is_archived: Option<bool>,
    pub notification_level: Option<String>,
    pub muted_until: Option<Option<String>>,
}
