//! Contacts, pinned messages and forwarding for the terminal client (TG-907): the same
//! endpoints the Web client uses, authorised server-side.

use reqwest::Method;
use serde::Deserialize;
use uuid::Uuid;

use crate::client_api::{ApiClient, ApiResult};

#[derive(Clone, Debug, Deserialize)]
pub struct Friend {
    pub id: Uuid,
    pub username: String,
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub remark: String,
}

impl Friend {
    /// Telegram's contact name: remark, else display name, else @username.
    pub fn name(&self) -> String {
        [&self.remark, &self.display_name]
            .into_iter()
            .find(|value| !value.trim().is_empty())
            .cloned()
            .unwrap_or_else(|| self.username.clone())
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct FriendStatus {
    pub user_id: Uuid,
    pub status: serde_json::Value,
}

#[derive(Clone, Debug, Deserialize)]
pub struct RequestUser {
    pub id: Uuid,
    pub username: String,
    #[serde(default)]
    pub display_name: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct FriendRequest {
    pub user: RequestUser,
}

#[derive(Clone, Debug, Deserialize)]
pub struct FoundUser {
    pub id: Uuid,
    pub username: String,
    #[serde(default)]
    pub relationship: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct PinnedMessage {
    pub id: Uuid,
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub recalled_at: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ChatPin {
    pub message: PinnedMessage,
    pub pinned_at: String,
}

#[derive(Clone, Debug, Deserialize)]
struct DirectChat {
    room_id: Uuid,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ForwardResult {
    #[serde(default)]
    pub forwarded_message_id: Option<Uuid>,
    #[serde(default)]
    pub skipped_reason: Option<String>,
}

impl ApiClient {
    pub async fn friends(&self) -> ApiResult<Vec<Friend>> {
        self.json(self.auth(Method::GET, "/api/friends")?, "load friends")
            .await
    }

    pub async fn friend_statuses(&self) -> ApiResult<Vec<FriendStatus>> {
        self.json(
            self.auth(Method::GET, "/api/friends/statuses")?,
            "load friend statuses",
        )
        .await
    }

    pub async fn incoming_requests(&self) -> ApiResult<Vec<FriendRequest>> {
        self.json(
            self.auth(Method::GET, "/api/friend-requests")?
                .query(&[("direction", "incoming")]),
            "load friend requests",
        )
        .await
    }

    pub async fn respond_request(&self, user_id: Uuid, accept: bool) -> ApiResult<()> {
        self.empty(
            self.auth(Method::PATCH, &format!("/api/friend-requests/{user_id}"))?
                .json(&serde_json::json!({ "action": if accept { "accept" } else { "decline" } })),
            "answer friend request",
        )
        .await
    }

    /// Find `username` exactly and send it a friend request (accepting theirs if pending).
    pub async fn add_contact(&self, username: &str) -> ApiResult<String> {
        let wanted = username.trim().trim_start_matches('@').to_string();
        let found: Vec<FoundUser> = self
            .json(
                self.auth(Method::GET, "/api/users/search")?
                    .query(&[("q", wanted.as_str())]),
                "search users",
            )
            .await?;
        let Some(user) = found
            .into_iter()
            .find(|user| user.username.eq_ignore_ascii_case(&wanted))
        else {
            return Ok(format!("No user @{wanted}"));
        };
        if user.relationship == "friend" {
            return Ok(format!("@{} is already a contact", user.username));
        }
        self.empty(
            self.auth(Method::POST, "/api/friend-requests")?
                .json(&serde_json::json!({ "user_id": user.id })),
            "send friend request",
        )
        .await?;
        Ok(format!("Request sent to @{}", user.username))
    }

    pub async fn open_direct_chat(&self, user_id: Uuid) -> ApiResult<Uuid> {
        let chat: DirectChat = self
            .json(
                self.auth(Method::POST, "/api/direct-chats")?
                    .json(&serde_json::json!({ "user_id": user_id })),
                "open private chat",
            )
            .await?;
        Ok(chat.room_id)
    }

    pub async fn pins(&self, room_id: Uuid) -> ApiResult<Vec<ChatPin>> {
        self.json(
            self.auth(Method::GET, &format!("/api/chats/{room_id}/pins"))?,
            "load pinned messages",
        )
        .await
    }

    pub async fn set_pinned(&self, room_id: Uuid, message_id: Uuid, pinned: bool) -> ApiResult<()> {
        let method = if pinned { Method::POST } else { Method::DELETE };
        self.empty(
            self.auth(method, &format!("/api/chats/{room_id}/pins/{message_id}"))?,
            if pinned {
                "pin message"
            } else {
                "unpin message"
            },
        )
        .await
    }

    pub async fn forward(
        &self,
        message_id: Uuid,
        target_room_id: Uuid,
    ) -> ApiResult<ForwardResult> {
        let results: Vec<ForwardResult> = self
            .json(
                self.auth(Method::POST, "/api/messages/forward")?
                    .json(&serde_json::json!({
                        "message_ids": [message_id],
                        "target_room_ids": [target_room_id],
                    })),
                "forward message",
            )
            .await?;
        results
            .into_iter()
            .next()
            .ok_or_else(|| crate::client_api::ApiError::other("forward returned no result"))
    }
}

/// `在线`-style line for the contacts list, from the server's `UserStatus` JSON.
pub fn status_text(status: Option<&serde_json::Value>) -> String {
    let Some(status) = status else {
        return "last seen a long time ago".into();
    };
    match status["kind"].as_str() {
        Some("online") => "online".into(),
        Some("offline") => status["last_seen"]
            .as_str()
            .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
            .map(|seen| {
                format!(
                    "last seen {}",
                    seen.with_timezone(&chrono::Local).format("%m-%d %H:%M")
                )
            })
            .unwrap_or_else(|| "offline".into()),
        Some("recently") => "last seen recently".into(),
        Some("within_week") => "last seen within a week".into(),
        Some("within_month") => "last seen within a month".into(),
        _ => "last seen a long time ago".into(),
    }
}
