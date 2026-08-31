//! Search, notifications, favorites, and AI bindings for the terminal client.

use reqwest::Method;
use uuid::Uuid;

use crate::client_api::{
    AiRun, AiThread, AiThreadMessage, ApiClient, ApiResult, Favorite, NotificationPage, SearchPage,
};

impl ApiClient {
    pub async fn search(&self, query: &str) -> ApiResult<SearchPage> {
        self.json(
            self.auth(Method::GET, "/api/messages/search")?
                .query(&[("q", query), ("limit", "50")]),
            "search messages",
        )
        .await
    }

    pub async fn notifications(&self) -> ApiResult<NotificationPage> {
        self.json(
            self.auth(Method::GET, "/api/notifications")?
                .query(&[("limit", "50")]),
            "load notifications",
        )
        .await
    }

    pub async fn mark_notification_read(&self, id: &str) -> ApiResult<()> {
        self.empty(
            self.auth(
                Method::POST,
                &format!("/api/notifications/{}/read", encode_component(id)),
            )?,
            "mark notification read",
        )
        .await
    }

    pub async fn mark_all_notifications_read(&self) -> ApiResult<()> {
        self.empty(
            self.auth(Method::POST, "/api/notifications/read-all")?,
            "mark all notifications read",
        )
        .await
    }

    pub async fn favorites(&self) -> ApiResult<Vec<Favorite>> {
        self.json(self.auth(Method::GET, "/api/favorites")?, "load favorites")
            .await
    }

    pub async fn create_favorite(&self, title: &str, content: &str) -> ApiResult<Favorite> {
        self.json(
            self.auth(Method::POST, "/api/favorites")?
                .json(&serde_json::json!({ "title": title, "content": content })),
            "create favorite",
        )
        .await
    }

    pub async fn update_favorite(
        &self,
        id: Uuid,
        version: i64,
        title: &str,
        content: &str,
    ) -> ApiResult<Favorite> {
        self.json(
            self.auth(Method::PUT, &format!("/api/favorites/{id}"))?
                .json(&serde_json::json!({
                    "version": version,
                    "title": title,
                    "content": content
                })),
            "update favorite",
        )
        .await
    }

    pub async fn favorite_message(&self, message_id: Uuid) -> ApiResult<Vec<Favorite>> {
        self.json(
            self.auth(Method::POST, "/api/favorites/messages")?
                .json(&serde_json::json!({ "message_ids": [message_id] })),
            "favorite message",
        )
        .await
    }

    pub async fn delete_favorite(&self, id: Uuid) -> ApiResult<()> {
        self.empty(
            self.auth(Method::DELETE, &format!("/api/favorites/{id}"))?,
            "delete favorite",
        )
        .await
    }

    pub async fn ai_threads(&self) -> ApiResult<Vec<AiThread>> {
        self.json(
            self.auth(Method::GET, "/api/ai/threads")?,
            "load AI threads",
        )
        .await
    }

    pub async fn create_ai_thread(&self, room_id: Option<Uuid>) -> ApiResult<AiThread> {
        self.json(
            self.auth(Method::POST, "/api/ai/threads")?
                .json(&serde_json::json!({ "room_id": room_id })),
            "create AI thread",
        )
        .await
    }

    pub async fn ai_messages(&self, thread_id: Uuid) -> ApiResult<Vec<AiThreadMessage>> {
        self.json(
            self.auth(
                Method::GET,
                &format!("/api/ai/threads/{thread_id}/messages"),
            )?,
            "load AI messages",
        )
        .await
    }

    pub async fn create_ai_run(
        &self,
        thread_id: Uuid,
        question: &str,
        room_id: Option<Uuid>,
        room_password: Option<&str>,
    ) -> ApiResult<AiRun> {
        let mut request = self.auth(Method::POST, &format!("/api/ai/threads/{thread_id}/runs"))?;
        if let Some(password) = room_password {
            request = request.header("x-room-password", password);
        }
        self.json(
            request.json(&serde_json::json!({
                "question": question,
                "room_id": room_id,
                "model_option_id": null,
                "client_request_id": Uuid::new_v4(),
                "message_ids": []
            })),
            "start AI run",
        )
        .await
    }

    pub async fn ai_run(&self, run_id: Uuid) -> ApiResult<AiRun> {
        self.json(
            self.auth(Method::GET, &format!("/api/ai/runs/{run_id}"))?,
            "load AI run",
        )
        .await
    }
}

fn encode_component(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                output.push(byte as char);
            }
            _ => output.push_str(&format!("%{byte:02X}")),
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notification_ids_are_safe_path_segments() {
        assert_eq!(encode_component("reply/room:1"), "reply%2Froom%3A1");
    }
}
