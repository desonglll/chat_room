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
pub struct ChatSummary {
    pub id: Uuid,
    /// `/api/chats` calls it `title`; older servers also sent `name`.
    #[serde(alias = "name")]
    pub title: String,
    /// TG-602: `/api/chats` lists private chats too; the client filters on this.
    #[serde(default)]
    pub chat_type: Option<String>,
    #[serde(default)]
    pub has_password: bool,
    #[serde(default)]
    pub membership_status: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ChatMembership {
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
    /// TG-1103: `group`, `supergroup` or `channel` (folders filter by it).
    #[serde(default)]
    pub chat_type: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct MessagePreview {
    pub sender: String,
    pub content: String,
    #[serde(default)]
    pub recalled: bool,
    /// TG-802/TG-907: `voice`, `photo`, `poll`, … from the server; absent for text.
    #[serde(default)]
    pub media_kind: Option<String>,
}

impl MessagePreview {
    /// The one-line body: Telegram-style `[Voice message]`-type label for media, then caption.
    pub fn summary(&self) -> String {
        let label = self.media_kind.as_deref().map(media_label);
        match (label, self.content.trim()) {
            (Some(label), "") => format!("[{label}]"),
            (Some(label), text) => format!("[{label}] {text}"),
            (None, text) => text.to_string(),
        }
    }
}

/// English labels for the server's preview media kinds (the TUI's copy is English).
pub fn media_label(kind: &str) -> &'static str {
    match kind {
        "voice" => "Voice message",
        "video_note" => "Video message",
        "sticker" => "Sticker",
        "gif" => "GIF",
        "poll" => "Poll",
        "location" => "Location",
        "live_location" => "Live location",
        "contact" => "Contact",
        "album" => "Album",
        "photo" => "Photo",
        "video" => "Video",
        "audio" => "Audio",
        _ => "File",
    }
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

/// The preview kind of a live message's attachment, by MIME type (the chat frame carries no
/// `media_kind` for plain uploads).
pub fn attachment_media_kind(mime_type: &str) -> String {
    match mime_type.split('/').next() {
        _ if mime_type == "image/gif" => "gif",
        Some("image") => "photo",
        Some("video") => "video",
        Some("audio") => "audio",
        _ => "file",
    }
    .to_string()
}

#[cfg(test)]
mod preview_tests {
    use super::MessagePreview;

    fn preview(media_kind: Option<&str>, content: &str) -> MessagePreview {
        MessagePreview {
            sender: "a".into(),
            content: content.into(),
            recalled: false,
            media_kind: media_kind.map(str::to_string),
        }
    }

    #[test]
    fn media_previews_carry_a_label() {
        assert_eq!(preview(Some("voice"), "").summary(), "[Voice message]");
        assert_eq!(preview(Some("poll"), "Lunch?").summary(), "[Poll] Lunch?");
        assert_eq!(preview(None, "hi").summary(), "hi");
        assert_eq!(super::attachment_media_kind("image/gif"), "gif");
        assert_eq!(super::attachment_media_kind("application/pdf"), "file");
    }
}
