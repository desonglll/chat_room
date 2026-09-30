//! TG-206 public usernames ("公开链接"): a supergroup or channel can take a public handle,
//! be previewed by anyone signed in through that handle, and be joined through it.
//!
//! - Handles follow Telegram's rules: 5–32 characters of `a-z`, `0-9` and `_`, starting with a
//!   letter, no trailing or doubled underscore, not a reserved word. They are case-insensitive
//!   and stored lowercase, so the existing partial unique index enforces uniqueness; chats and
//!   user logins share one namespace (Telegram's `t.me/<name>`), so a user's login is taken too.
//! - Resolving a handle never reveals the chat's internal id to a non-member: the preview
//!   carries only the fields Telegram shows before joining, and joining goes through the handle.
//!   A member's preview includes the id so the client can open the chat.
//! - Setting a handle on a group upgrades it to a supergroup (TG-201 `apply_chat_capabilities`).
//!   A chat with a join password cannot be public.

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use super::capabilities::{CapabilityError, ChatCapabilityChange};
use super::chat_type::ChatType;
use super::membership_handlers::{request_join, require_permission, session_user};
use super::models::{ChatMembership, JoinChatRequest};
use crate::models::Chat;
use crate::state::{with_pool, AppState, SharedState};

pub const USERNAME_MIN: usize = 5;
pub const USERNAME_MAX: usize = 32;

/// Handles nobody may take: product routes, staff-looking names, Telegram's own.
const RESERVED: &[&str] = &[
    "admin",
    "administrator",
    "support",
    "help",
    "settings",
    "official",
    "system",
    "security",
    "telegram",
    "echogate",
    "echo_gate",
    "joinchat",
    "addstickers",
    "addemoji",
    "public",
    "share",
    "login",
    "logout",
    "signup",
    "register",
    "api",
    "chat",
    "chats",
    "channel",
    "channels",
    "group",
    "groups",
    "username",
    "everyone",
    "moderator",
    "root",
    "null",
];

/// Why a handle was refused (the wire code).
pub fn validate_username(raw: &str) -> Result<String, &'static str> {
    let name = raw.trim().to_ascii_lowercase();
    if name.len() < USERNAME_MIN {
        return Err("too_short");
    }
    if name.len() > USERNAME_MAX {
        return Err("too_long");
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
    {
        return Err("invalid_characters");
    }
    if !name.starts_with(|c: char| c.is_ascii_lowercase()) {
        return Err("must_start_with_letter");
    }
    if name.ends_with('_') || name.contains("__") {
        return Err("invalid_underscores");
    }
    if RESERVED.contains(&name.as_str()) {
        return Err("reserved");
    }
    Ok(name)
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UsernameRequest {
    /// The new handle, or `null` to make the chat private again.
    pub username: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct UsernameCheck {
    pub username: String,
    pub available: bool,
    /// `too_short`, `invalid_characters`, `reserved`, `taken` … when not available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<&'static str>,
}

/// What anyone signed in may see of a public chat through its handle.
#[derive(Debug, Serialize, ToSchema)]
pub struct PublicChatPreview {
    pub username: String,
    pub title: String,
    pub description: String,
    pub avatar_emoji: String,
    pub chat_type: ChatType,
    pub member_count: i64,
    /// Whether the caller is already an active member.
    pub is_member: bool,
    /// Present only for members (a non-member never learns the internal id).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chat_id: Option<Uuid>,
    /// Joining asks an admin first (the chat's join policy).
    pub requires_approval: bool,
}

impl AppState {
    /// The live public chat holding `username` (already normalised), if any.
    pub async fn chat_id_by_username(&self, username: &str) -> Result<Option<Uuid>, sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query_scalar("SELECT id FROM chats WHERE username = $1 AND deleted_at IS NULL")
                .bind(username)
                .fetch_optional(pool)
                .await
        })
    }

    /// Whether `username` is taken by another chat or by any user's login.
    pub async fn username_taken(
        &self,
        username: &str,
        except_chat: Option<Uuid>,
    ) -> Result<bool, sqlx::Error> {
        if let Some(owner) = self.chat_id_by_username(username).await? {
            if Some(owner) != except_chat {
                return Ok(true);
            }
        }
        let user: Option<Uuid> = with_pool!(self, |pool| {
            sqlx::query_scalar("SELECT id FROM users WHERE LOWER(username) = $1")
                .bind(username)
                .fetch_optional(pool)
                .await
        })?;
        Ok(user.is_some())
    }

    async fn public_chat(&self, raw: &str) -> Result<Option<Chat>, sqlx::Error> {
        let Ok(username) = validate_username(raw) else {
            return Ok(None);
        };
        let Some(id) = self.chat_id_by_username(&username).await? else {
            return Ok(None);
        };
        Ok(self
            .chat(id)
            .await
            .filter(|chat| chat.username.is_some() && !chat.has_password))
    }
}

#[utoipa::path(get, path = "/api/public-usernames/{username}/check",
    params(("username" = String, Path, description = "Candidate handle")),
    responses((status = 200, description = "Whether the handle can be taken", body = UsernameCheck)))]
pub async fn check_username(
    State(state): State<SharedState>,
    Path(username): Path<String>,
    headers: HeaderMap,
) -> Result<Json<UsernameCheck>, StatusCode> {
    session_user(&state, &headers).await?;
    let normalised = username.trim().to_ascii_lowercase();
    let reason = match validate_username(&username) {
        Err(reason) => Some(reason),
        Ok(name) if state.username_taken(&name, None).await.map_err(internal)? => Some("taken"),
        Ok(_) => None,
    };
    Ok(Json(UsernameCheck {
        username: normalised,
        available: reason.is_none(),
        reason,
    }))
}

#[utoipa::path(put, path = "/api/chats/{id}/username", params(("id" = Uuid, description = "Chat id")),
    request_body = UsernameRequest,
    responses((status = 200, description = "Handle set or cleared (a group becomes a supergroup)", body = Chat),
        (status = 400, description = "Invalid handle; body `{ error }`"),
        (status = 403, description = "Missing chat.info"),
        (status = 409, description = "Taken, or this chat cannot be public (private chat, join password)")))]
pub async fn put_username(
    State(state): State<SharedState>,
    Path(room_id): Path<Uuid>,
    headers: HeaderMap,
    Json(request): Json<UsernameRequest>,
) -> Result<Json<Chat>, (StatusCode, Json<serde_json::Value>)> {
    let fail =
        |status: StatusCode, reason: &str| (status, Json(serde_json::json!({ "error": reason })));
    let user = session_user(&state, &headers)
        .await
        .map_err(|status| fail(status, "unauthorized"))?;
    require_permission(&state, room_id, user.id, "chat.info")
        .await
        .map_err(|status| fail(status, "forbidden"))?;
    let chat = state
        .chat(room_id)
        .await
        .ok_or_else(|| fail(StatusCode::NOT_FOUND, "not_found"))?;
    let username = match request.username.as_deref().map(str::trim) {
        None | Some("") => None,
        Some(raw) => {
            let name =
                validate_username(raw).map_err(|reason| fail(StatusCode::BAD_REQUEST, reason))?;
            if chat.has_password {
                return Err(fail(StatusCode::CONFLICT, "password_protected"));
            }
            if state
                .username_taken(&name, Some(room_id))
                .await
                .map_err(|error| fail(internal(error), "internal"))?
            {
                return Err(fail(StatusCode::CONFLICT, "taken"));
            }
            Some(name)
        }
    };
    let change = ChatCapabilityChange {
        username: Some(username),
        ..Default::default()
    };
    match state.apply_chat_capabilities(room_id, change).await {
        Ok(outcome) => Ok(Json(outcome.chat)),
        Err(CapabilityError::NotFound) => Err(fail(StatusCode::NOT_FOUND, "not_found")),
        Err(CapabilityError::NotSupported(_)) => Err(fail(StatusCode::CONFLICT, "not_supported")),
        // Two admins racing for one handle: the unique index decides.
        Err(CapabilityError::Database(sqlx::Error::Database(error)))
            if error.is_unique_violation() =>
        {
            Err(fail(StatusCode::CONFLICT, "taken"))
        }
        Err(CapabilityError::Database(error)) => Err(fail(internal(error), "internal")),
    }
}

#[utoipa::path(get, path = "/api/public/{username}",
    params(("username" = String, Path, description = "Public handle")),
    responses((status = 200, description = "What a visitor may see before joining", body = PublicChatPreview),
        (status = 404, description = "No public chat with this handle")))]
pub async fn public_preview(
    State(state): State<SharedState>,
    Path(username): Path<String>,
    headers: HeaderMap,
) -> Result<Json<PublicChatPreview>, StatusCode> {
    let user = session_user(&state, &headers).await?;
    let chat = state
        .public_chat(&username)
        .await
        .map_err(internal)?
        .ok_or(StatusCode::NOT_FOUND)?;
    let is_member = matches!(
        state.membership_identity(chat.id, user.id).await.map_err(internal)?,
        Some((status, _)) if status == "active"
    );
    Ok(Json(PublicChatPreview {
        username: chat.username.clone().unwrap_or_default(),
        title: chat.title.clone(),
        description: chat.description.clone(),
        avatar_emoji: chat.avatar_emoji.clone(),
        chat_type: chat.chat_type,
        member_count: chat.member_count,
        is_member,
        chat_id: is_member.then_some(chat.id),
        requires_approval: chat.join_policy != "open",
    }))
}

#[utoipa::path(post, path = "/api/public/{username}/join",
    params(("username" = String, Path, description = "Public handle")),
    responses((status = 200, description = "Joined", body = ChatMembership),
        (status = 202, description = "Join request sent to the admins", body = ChatMembership),
        (status = 403, description = "Banned"), (status = 404, description = "No public chat with this handle")))]
pub async fn join_public(
    state: State<SharedState>,
    Path(username): Path<String>,
    headers: HeaderMap,
) -> Result<(StatusCode, Json<ChatMembership>), StatusCode> {
    let chat = state
        .public_chat(&username)
        .await
        .map_err(internal)?
        .ok_or(StatusCode::NOT_FOUND)?;
    // The ordinary join, so bans, locks, approval and the join policy stay in one place.
    request_join(
        state,
        Path(chat.id),
        headers,
        Json(JoinChatRequest { password: None }),
    )
    .await
}

fn internal(error: sqlx::Error) -> StatusCode {
    tracing::error!("public handles: {error}");
    StatusCode::INTERNAL_SERVER_ERROR
}

#[cfg(test)]
mod tests {
    use super::validate_username;

    #[test]
    fn handles_follow_telegrams_rules() {
        assert_eq!(
            validate_username("  Rust_Lang  "),
            Ok("rust_lang".to_string())
        );
        assert_eq!(validate_username("abcd"), Err("too_short"));
        assert_eq!(validate_username(&"a".repeat(33)), Err("too_long"));
        assert_eq!(validate_username("rust-lang"), Err("invalid_characters"));
        assert_eq!(validate_username("中文名字很长"), Err("invalid_characters"));
        assert_eq!(
            validate_username("1rustlang"),
            Err("must_start_with_letter")
        );
        assert_eq!(
            validate_username("_rustlang"),
            Err("must_start_with_letter")
        );
        assert_eq!(validate_username("rustlang_"), Err("invalid_underscores"));
        assert_eq!(validate_username("rust__lang"), Err("invalid_underscores"));
        assert_eq!(validate_username("Support"), Err("reserved"));
        assert_eq!(validate_username("joinchat"), Err("reserved"));
    }
}
