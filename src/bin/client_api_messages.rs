//! TG-1205: scheduled messages, in-chat search and joining by invite link for the terminal client
//! (the same endpoints the Web client uses).

use reqwest::Method;
use serde::Deserialize;
use uuid::Uuid;

use crate::client_api::{ApiClient, ApiResult};

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ScheduledMessage {
    pub id: Uuid,
    pub content: String,
    pub scheduled_at: String,
    #[serde(default)]
    pub silent: bool,
}

/// One in-chat search hit (a subset of the server's stored message).
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct FoundMessage {
    pub id: Uuid,
    pub sender: String,
    pub content: String,
    pub created_at: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct InviteJoin {
    /// `active` (joined or already a member) or `pending` (waiting for an admin).
    pub status: String,
    #[serde(default)]
    pub chat_id: Option<Uuid>,
}

impl ApiClient {
    pub async fn scheduled_messages(&self, room_id: Uuid) -> ApiResult<Vec<ScheduledMessage>> {
        self.json(
            self.auth(
                Method::GET,
                &format!("/api/chats/{room_id}/scheduled-messages"),
            )?,
            "load scheduled messages",
        )
        .await
    }

    pub async fn schedule_message(
        &self,
        room_id: Uuid,
        content: &str,
        at: chrono::DateTime<chrono::Utc>,
        silent: bool,
    ) -> ApiResult<ScheduledMessage> {
        self.json(
            self.auth(
                Method::POST,
                &format!("/api/chats/{room_id}/scheduled-messages"),
            )?
            .json(&serde_json::json!({
                "content": content,
                "scheduled_at": at.to_rfc3339(),
                "silent": silent
            })),
            "schedule message",
        )
        .await
    }

    pub async fn cancel_scheduled(&self, room_id: Uuid, id: Uuid) -> ApiResult<()> {
        self.empty(
            self.auth(
                Method::DELETE,
                &format!("/api/chats/{room_id}/scheduled-messages/{id}"),
            )?,
            "cancel scheduled message",
        )
        .await
    }

    pub async fn send_scheduled_now(&self, room_id: Uuid, id: Uuid) -> ApiResult<()> {
        let _: serde_json::Value = self
            .json(
                self.auth(
                    Method::POST,
                    &format!("/api/chats/{room_id}/scheduled-messages/{id}/send-now"),
                )?,
                "send scheduled message",
            )
            .await?;
        Ok(())
    }

    pub async fn search_chat(
        &self,
        room_id: Uuid,
        password: Option<&str>,
        query: &str,
    ) -> ApiResult<Vec<FoundMessage>> {
        let mut request = self
            .auth(
                Method::GET,
                &format!("/api/chats/{room_id}/messages/search"),
            )?
            .query(&[("q", query)]);
        if let Some(password) = password {
            request = request.header("x-room-password", password);
        }
        self.json(request, "search chat").await
    }

    pub async fn join_invite(&self, token: &str) -> ApiResult<InviteJoin> {
        self.json(
            self.auth(Method::POST, &format!("/api/invite-links/{token}/join"))?,
            "join by invite link",
        )
        .await
    }
}

/// The token from a pasted invite: a full link (`https://host/joinchat/TOKEN`, `…/+TOKEN`), a
/// path, or the bare token. `None` when it cannot be a token (the server's own plausibility rule).
pub fn invite_token(input: &str) -> Option<String> {
    let trimmed = input.trim().trim_end_matches('/');
    let last = trimmed
        .rsplit(['/', '+'])
        .next()
        .unwrap_or(trimmed)
        .split(['?', '#'])
        .next()
        .unwrap_or_default();
    ((16..=64).contains(&last.len())
        && last
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_'))
    .then(|| last.to_string())
}

#[cfg(test)]
mod tests {
    use super::invite_token;

    #[test]
    fn invite_tokens_are_taken_from_links_paths_and_bare_input() {
        let token = "AbCdEfGhIjKlMnOp_-12";
        for input in [
            format!("https://chat.example/joinchat/{token}"),
            format!("https://chat.example/joinchat/{token}/"),
            format!("http://127.0.0.1:3000/+{token}?ref=x"),
            format!("/joinchat/{token}"),
            format!("  {token} "),
        ] {
            assert_eq!(invite_token(&input).as_deref(), Some(token), "{input}");
        }
        assert_eq!(invite_token("https://chat.example/joinchat/short"), None);
        assert_eq!(invite_token("bad token with spaces!!"), None);
        assert_eq!(invite_token(""), None);
    }
}
