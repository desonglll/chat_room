//! Typed HTTP bindings used by both the compatibility CLI and the TUI.

use std::fmt;

use reqwest::{Method, RequestBuilder, StatusCode};
use serde::de::DeserializeOwned;
use uuid::Uuid;

pub use crate::client_api_models::*;

#[derive(Clone)]
pub struct ApiClient {
    base: String,
    http: reqwest::Client,
    token: Option<Uuid>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApiError {
    pub status: Option<StatusCode>,
    message: String,
}

impl ApiError {
    fn transport(operation: &str, error: reqwest::Error) -> Self {
        Self {
            status: None,
            message: format!("{operation}: {error}"),
        }
    }

    fn response(operation: &str, status: StatusCode) -> Self {
        let detail = match status {
            StatusCode::BAD_REQUEST => "request was rejected",
            StatusCode::UNAUTHORIZED => "session expired or credentials are incorrect",
            StatusCode::FORBIDDEN => "operation is not allowed",
            StatusCode::NOT_FOUND => "resource was not found",
            StatusCode::CONFLICT => "resource changed or already exists",
            StatusCode::PAYLOAD_TOO_LARGE => "payload is too large",
            StatusCode::TOO_MANY_REQUESTS => "request limit reached; try again later",
            StatusCode::SERVICE_UNAVAILABLE => "service is temporarily unavailable",
            _ => "server returned an unexpected response",
        };
        Self {
            status: Some(status),
            message: format!("{operation}: {detail} ({status})"),
        }
    }

    pub fn is_unauthorized(&self) -> bool {
        self.status == Some(StatusCode::UNAUTHORIZED)
    }
}

impl fmt::Display for ApiError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for ApiError {}

pub type ApiResult<T> = Result<T, ApiError>;

impl ApiClient {
    pub fn new(base: &str, token: Option<Uuid>) -> Self {
        Self {
            base: base.trim_end_matches('/').to_string(),
            http: reqwest::Client::new(),
            token,
        }
    }

    pub async fn authenticate(
        &self,
        register: bool,
        username: &str,
        password: &str,
    ) -> ApiResult<AuthSession> {
        let endpoint = if register { "register" } else { "login" };
        self.json(
            self.http
                .post(format!("{}/api/users/{endpoint}", self.base))
                .json(&serde_json::json!({ "username": username, "password": password })),
            "authenticate",
        )
        .await
    }

    pub async fn validate_session(&self) -> ApiResult<()> {
        let _: serde_json::Value = self
            .json(self.auth(Method::GET, "/api/users/me")?, "validate session")
            .await?;
        Ok(())
    }

    pub async fn logout(&self) -> ApiResult<()> {
        self.empty(self.auth(Method::POST, "/api/users/logout")?, "log out")
            .await
    }

    pub async fn conversations(&self) -> ApiResult<Vec<Conversation>> {
        self.json(
            self.auth(Method::GET, "/api/conversations")?,
            "load conversations",
        )
        .await
    }

    pub async fn rooms(&self) -> ApiResult<Vec<RoomSummary>> {
        self.json(self.auth(Method::GET, "/api/rooms")?, "load rooms")
            .await
    }

    pub async fn discover_rooms(&self) -> ApiResult<Vec<RoomSummary>> {
        self.json(
            self.auth(Method::GET, "/api/rooms/discover")?,
            "discover rooms",
        )
        .await
    }

    pub async fn create_room(&self, name: &str, password: Option<&str>) -> ApiResult<RoomSummary> {
        self.json(
            self.auth(Method::POST, "/api/rooms")?
                .json(&serde_json::json!({
                    "name": name,
                    "password": password.filter(|value| !value.is_empty()),
                    "join_policy": "open",
                    "avatar_emoji": "",
                    "description": ""
                })),
            "create room",
        )
        .await
    }

    pub async fn join_room(
        &self,
        room_id: Uuid,
        password: Option<&str>,
    ) -> ApiResult<RoomMembership> {
        self.json(
            self.auth(Method::POST, &format!("/api/rooms/{room_id}/join-requests"))?
                .json(&serde_json::json!({ "password": password })),
            "join room",
        )
        .await
    }

    pub async fn update_preferences(
        &self,
        room_id: Uuid,
        patch: PreferencePatch,
    ) -> ApiResult<ConversationPreferences> {
        let payload = preference_payload(patch);
        self.json(
            self.auth(
                Method::PATCH,
                &format!("/api/conversations/{room_id}/preferences"),
            )?
            .json(&payload),
            "update conversation preferences",
        )
        .await
    }

    pub(crate) fn auth(&self, method: Method, path: &str) -> ApiResult<RequestBuilder> {
        let token = self.token.ok_or_else(|| ApiError {
            status: Some(StatusCode::UNAUTHORIZED),
            message: "not logged in".into(),
        })?;
        Ok(self
            .http
            .request(method, format!("{}{}", self.base, path))
            .bearer_auth(token))
    }

    pub(crate) async fn json<T: DeserializeOwned>(
        &self,
        request: RequestBuilder,
        operation: &str,
    ) -> ApiResult<T> {
        let response = request
            .send()
            .await
            .map_err(|error| ApiError::transport(operation, error))?;
        let status = response.status();
        if !status.is_success() {
            return Err(ApiError::response(operation, status));
        }
        response
            .json()
            .await
            .map_err(|error| ApiError::transport(&format!("decode {operation}"), error))
    }

    pub(crate) async fn empty(&self, request: RequestBuilder, operation: &str) -> ApiResult<()> {
        let response = request
            .send()
            .await
            .map_err(|error| ApiError::transport(operation, error))?;
        if response.status().is_success() {
            Ok(())
        } else {
            Err(ApiError::response(operation, response.status()))
        }
    }
}

fn preference_payload(patch: PreferencePatch) -> serde_json::Map<String, serde_json::Value> {
    let mut payload = serde_json::Map::new();
    if let Some(value) = patch.is_pinned {
        payload.insert("is_pinned".into(), value.into());
    }
    if let Some(value) = patch.is_archived {
        payload.insert("is_archived".into(), value.into());
    }
    if let Some(value) = patch.notification_level {
        payload.insert("notification_level".into(), value.into());
    }
    if let Some(value) = patch.muted_until {
        payload.insert(
            "muted_until".into(),
            value.map_or(serde_json::Value::Null, serde_json::Value::String),
        );
    }
    payload
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_conversation_payloads_receive_preference_defaults() {
        let conversation: Conversation = serde_json::from_value(serde_json::json!({
            "room_id": Uuid::nil(),
            "kind": "group",
            "title": "General"
        }))
        .unwrap();
        assert_eq!(conversation.preferences.notification_level, "all");
        assert!(!conversation.preferences.is_pinned);
    }

    #[test]
    fn preference_patch_does_not_clear_unrelated_mute() {
        let payload = preference_payload(PreferencePatch {
            is_pinned: Some(true),
            ..PreferencePatch::default()
        });
        assert_eq!(payload.len(), 1);
        assert_eq!(payload["is_pinned"], true);
        assert!(!payload.contains_key("muted_until"));

        let unmute = preference_payload(PreferencePatch {
            muted_until: Some(None),
            ..PreferencePatch::default()
        });
        assert!(unmute["muted_until"].is_null());
    }
}
