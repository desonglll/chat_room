//! Creating a chat, and the validation rules every chat write shares.

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use chrono::Utc;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::ChatType;
use crate::admin_system_lock::require_chat_rooms_unlocked;
use crate::models::{Chat, CreateChatRequest};
use crate::state::SharedState;
use crate::user_handlers::bearer_token;

const MAX_CHAT_TITLE_CHARS: usize = 80;
pub(crate) const MAX_PASSWORD_CHARS: usize = 256;
const MAX_CHAT_AVATAR_CHARS: usize = 8;
const MAX_CHAT_DESCRIPTION_CHARS: usize = 300;
/// 64 bits, taken from the random bits of a v4 UUID so that no new dependency is needed for
/// a value the migration generates with `hex(randomblob(8))`.
const ACCESS_HASH_HEX_CHARS: usize = 16;

pub(crate) fn valid_chat_avatar(value: &str) -> bool {
    value.chars().count() <= MAX_CHAT_AVATAR_CHARS && !value.chars().any(char::is_control)
}

pub(crate) fn valid_chat_description(value: &str) -> bool {
    value.chars().count() <= MAX_CHAT_DESCRIPTION_CHARS && !value.chars().any(char::is_control)
}

pub(crate) fn hash_password(password: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(password.as_bytes());
    hex::encode(hasher.finalize())
}

pub(crate) fn valid_chat_title(title: &str) -> bool {
    !title.is_empty() && title.chars().count() <= MAX_CHAT_TITLE_CHARS
}

pub(crate) fn authorize_chat(chat: &Chat, supplied: Option<&str>) -> bool {
    !chat.has_password
        || supplied.is_some_and(|password| hash_password(password) == chat.password_hash)
}

/// A public handle must be resolvable without revealing the chat's row id.
fn generate_access_hash() -> String {
    Uuid::new_v4()
        .simple()
        .to_string()
        .chars()
        .take(ACCESS_HASH_HEX_CHARS)
        .collect()
}

/// Create a chat. Omit the password for a public chat.
#[utoipa::path(
    post,
    path = "/api/chats",
    request_body = CreateChatRequest,
    responses(
        (status = 201, description = "Chat created", body = Chat),
        (status = 400, description = "Invalid chat title, password or chat type"),
        (status = 409, description = "Chat title already exists"),
        (status = 500, description = "Database error")
    )
)]
pub async fn create_chat(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(req): Json<CreateChatRequest>,
) -> Result<(StatusCode, Response), StatusCode> {
    let token = bearer_token(&headers)?;
    let creator = state
        .session_user(token)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::UNAUTHORIZED)?;
    require_chat_rooms_unlocked(&state).await?;
    let title = req.title.trim().to_string();
    if !valid_chat_title(&title) {
        return Err(StatusCode::BAD_REQUEST);
    }
    if req
        .password
        .as_deref()
        .is_some_and(|password| password.chars().count() > MAX_PASSWORD_CHARS)
    {
        return Err(StatusCode::BAD_REQUEST);
    }
    let join_policy = req.join_policy.as_deref().unwrap_or("open");
    if !matches!(join_policy, "open" | "approval") {
        return Err(StatusCode::BAD_REQUEST);
    }
    let avatar_emoji = req.avatar_emoji.as_deref().unwrap_or("").trim().to_string();
    let description = req.description.as_deref().unwrap_or("").trim().to_string();
    if !valid_chat_avatar(&avatar_emoji) || !valid_chat_description(&description) {
        return Err(StatusCode::BAD_REQUEST);
    }

    let chat_type = match req.chat_type.unwrap_or(ChatType::Group) {
        chat_type @ (ChatType::Group | ChatType::Channel) => chat_type,
        ChatType::Private | ChatType::Supergroup => return Err(StatusCode::BAD_REQUEST),
    };
    let signatures_enabled =
        chat_type == ChatType::Channel && req.signatures_enabled.unwrap_or(false);

    let id = Uuid::new_v4();
    let (password_hash, has_password) = match req.password.as_deref() {
        Some(password) if !password.is_empty() => (hash_password(password), true),
        _ => (String::new(), false),
    };

    // Every field is spelled out rather than leaning on `Chat::default()`: this is the one
    // place that decides what a brand-new chat is, and a silent default here would be a
    // silent product decision.
    let chat = Chat {
        id,
        // A small group or (TG-202) a channel. `private` is opened by chats::private_chats
        // between two friends; a group reaches supergroup only through the one-way upgrade
        // in chats::supergroup_upgrade.
        chat_type,
        title,
        password_hash,
        has_password,
        creator_user_id: Some(creator.id),
        join_policy: join_policy.to_string(),
        avatar_emoji,
        description,
        username: None,
        access_hash: generate_access_hash(),
        is_forum: false,
        linked_chat_id: None,
        slow_mode_seconds: 0,
        auto_delete_seconds: 0,
        signatures_enabled,
        history_visible_to_new_members: true,
        member_count: 1,
        membership_status: Some("active".into()),
        membership_role: Some("owner".into()),
        unread_count: 0,
        created_at: Utc::now(),
    };

    match state.create_chat_with_owner(chat.clone(), creator.id).await {
        Ok(()) => Ok((StatusCode::CREATED, Json(chat).into_response())),
        Err(sqlx::Error::Database(error)) if error.is_unique_violation() => {
            Err(StatusCode::CONFLICT)
        }
        Err(error) => {
            tracing::error!("create chat in SQLite failed: {}", error);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}
