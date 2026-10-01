//! TG-1103: poll voting and chat folders for the terminal client (same endpoints as Web).

use reqwest::Method;
use serde::Deserialize;
use uuid::Uuid;

use crate::client_api::{ApiClient, ApiResult, Conversation};
use crate::client_chat_media::Poll;

/// A chat folder's rules (TG-501); membership is evaluated client-side, like the Web client.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ChatFolder {
    pub id: Uuid,
    pub title: String,
    #[serde(default)]
    pub emoji: String,
    #[serde(default)]
    pub include_types: Vec<String>,
    #[serde(default)]
    pub include_chat_ids: Vec<Uuid>,
    #[serde(default)]
    pub exclude_chat_ids: Vec<Uuid>,
    #[serde(default)]
    pub exclude_muted: bool,
    #[serde(default)]
    pub exclude_read: bool,
    #[serde(default)]
    pub exclude_archived: bool,
}

impl ChatFolder {
    /// The Web client's rule (`packages/core/src/domain/chatFolders.ts`): not excluded, included
    /// by id or by type, then the exclusion flags apply even to explicitly included chats.
    pub fn contains(&self, chat: &Conversation, now: chrono::DateTime<chrono::Utc>) -> bool {
        if self.exclude_chat_ids.contains(&chat.room_id) {
            return false;
        }
        let kind = if chat.kind == "direct" {
            "private"
        } else if chat
            .group
            .as_ref()
            .is_some_and(|group| group.chat_type == "channel")
        {
            "channels"
        } else {
            "groups"
        };
        if !self.include_chat_ids.contains(&chat.room_id)
            && !self.include_types.iter().any(|t| t == kind)
        {
            return false;
        }
        let muted = chat.preferences.notification_level == "none"
            || chat
                .preferences
                .muted_until
                .as_deref()
                .and_then(|until| chrono::DateTime::parse_from_rfc3339(until).ok())
                .is_some_and(|until| until > now);
        !(self.exclude_muted && muted
            || self.exclude_read && chat.unread_count == 0
            || self.exclude_archived && chat.preferences.is_archived)
    }
}

impl ApiClient {
    /// Vote with option indexes (0-based); an empty list retracts the vote.
    pub async fn vote(&self, message_id: Uuid, options: &[u32]) -> ApiResult<Poll> {
        let path = format!("/api/polls/{message_id}/votes");
        if options.is_empty() {
            self.json(self.auth(Method::DELETE, &path)?, "retract vote")
                .await
        } else {
            self.json(
                self.auth(Method::POST, &path)?
                    .json(&serde_json::json!({ "options": options })),
                "vote",
            )
            .await
        }
    }

    pub async fn folders(&self) -> ApiResult<Vec<ChatFolder>> {
        self.json(
            self.auth(Method::GET, "/api/users/me/folders")?,
            "load folders",
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client_api::{ConversationGroup, ConversationPreferences};

    fn chat(kind: &str, chat_type: &str, unread: i64) -> Conversation {
        Conversation {
            room_id: Uuid::new_v4(),
            kind: kind.into(),
            title: "c".into(),
            unread_count: unread,
            group: (kind == "group").then(|| ConversationGroup {
                has_password: false,
                chat_type: chat_type.into(),
            }),
            preferences: ConversationPreferences::default(),
            last_message: None,
        }
    }

    #[test]
    fn folder_rules_match_the_web_client() {
        let now = chrono::Utc::now();
        let (direct, group, channel) = (
            chat("direct", "", 1),
            chat("group", "group", 0),
            chat("group", "channel", 2),
        );
        let mut folder = ChatFolder {
            id: Uuid::new_v4(),
            title: "Work".into(),
            emoji: String::new(),
            include_types: vec!["groups".into(), "channels".into()],
            include_chat_ids: vec![],
            exclude_chat_ids: vec![channel.room_id],
            exclude_muted: false,
            exclude_read: false,
            exclude_archived: false,
        };
        assert!(!folder.contains(&direct, now));
        assert!(folder.contains(&group, now));
        assert!(!folder.contains(&channel, now), "excluded by id");
        folder.include_chat_ids.push(direct.room_id);
        assert!(folder.contains(&direct, now), "included by id");
        folder.exclude_read = true;
        assert!(!folder.contains(&group, now), "read chats are excluded");
        assert!(folder.contains(&direct, now));
    }
}
