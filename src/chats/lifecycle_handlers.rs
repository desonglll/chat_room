//! Editing and soft-deleting one chat.
//!
//! Split out of `handlers` so that creation, mutation and history each stay readable and under
//! the 350-line warning in `scripts/check_file_sizes.py`.

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::Response,
    Json,
};
use uuid::Uuid;

use super::handlers::{
    hash_password, valid_chat_avatar, valid_chat_description, valid_chat_title, MAX_PASSWORD_CHARS,
};
use super::membership_handlers::reject_private_chat;
use super::ApiDialect;
use crate::models::{Chat, UpdateChatRequest};
use crate::state::SharedState;
use crate::user_handlers::bearer_token;

/// Rename a chat or change its password. Private chats require the current password.
#[utoipa::path(
    patch,
    path = "/api/chats/{id}",
    request_body = UpdateChatRequest,
    responses(
        (status = 200, description = "Chat updated", body = Chat),
        (status = 400, description = "Invalid or empty update"),
        (status = 401, description = "Incorrect current password"),
        (status = 404, description = "Chat not found"),
        (status = 409, description = "Title conflict or concurrent update"),
        (status = 500, description = "Database error")
    )
)]
pub async fn update_chat(
    State(state): State<SharedState>,
    Path(id): Path<Uuid>,
    dialect: ApiDialect,
    headers: HeaderMap,
    Json(req): Json<UpdateChatRequest>,
) -> Result<Response, StatusCode> {
    reject_private_chat(&state, id).await?;
    if req.title.is_none() && req.new_password.is_none() && req.join_policy.is_none() {
        return Err(StatusCode::BAD_REQUEST);
    }

    let token = bearer_token(&headers)?;
    let user = state
        .session_user(token)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::UNAUTHORIZED)?;
    if !state
        .has_chat_permission(id, user.id, "room.settings")
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    {
        return Err(StatusCode::FORBIDDEN);
    }
    let previous = state.chat(id).await.ok_or(StatusCode::NOT_FOUND)?;

    let title = req
        .title
        .as_deref()
        .map(str::trim)
        .unwrap_or(&previous.title)
        .to_string();
    if !valid_chat_title(&title)
        || req
            .new_password
            .as_deref()
            .is_some_and(|password| password.chars().count() > MAX_PASSWORD_CHARS)
    {
        return Err(StatusCode::BAD_REQUEST);
    }
    let join_policy = req.join_policy.as_deref().unwrap_or(&previous.join_policy);
    if !matches!(join_policy, "open" | "approval") {
        return Err(StatusCode::BAD_REQUEST);
    }
    let avatar_emoji = req
        .avatar_emoji
        .as_deref()
        .map(str::trim)
        .unwrap_or(&previous.avatar_emoji)
        .to_string();
    let description = req
        .description
        .as_deref()
        .map(str::trim)
        .unwrap_or(&previous.description)
        .to_string();
    if !valid_chat_avatar(&avatar_emoji) || !valid_chat_description(&description) {
        return Err(StatusCode::BAD_REQUEST);
    }

    let password_hash = req
        .new_password
        .as_deref()
        .map(|password| {
            if password.is_empty() {
                String::new()
            } else {
                hash_password(password)
            }
        })
        .unwrap_or_else(|| previous.password_hash.clone());
    let password_changed = password_hash != previous.password_hash;
    let updated = Chat {
        id,
        title,
        password_hash,
        has_password: req
            .new_password
            .as_ref()
            .map_or(previous.has_password, |password| !password.is_empty()),
        creator_user_id: previous.creator_user_id,
        join_policy: join_policy.to_string(),
        avatar_emoji,
        description,
        membership_status: Some("active".into()),
        membership_role: state
            .membership_identity(id, user.id)
            .await
            .ok()
            .flatten()
            .map(|(_, role)| role),
        unread_count: previous.unread_count,
        created_at: previous.created_at,
        // Everything the four chat types add is settings state that this endpoint does not
        // expose yet; M2 owns the handlers that change it.
        ..previous.clone()
    };

    match state.update_chat(&previous, updated.clone()).await {
        Ok(true) => {
            if password_changed {
                state
                    // Frozen wire value: web/src/roomSystemEvents.ts stored-password cleanup.
                    .restart_chat_connections(id, "room password changed")
                    .await;
            } else if updated.title != previous.title {
                state
                    .broadcast(
                        id,
                        crate::models::ChatMessage::System {
                            // Frozen wire value: web/src/roomSystemEvents.ts list refresh.
                            content: format!("room renamed to {}", updated.title),
                            members: None,
                            participants: None,
                        },
                    )
                    .await;
            }
            Ok(dialect.chat(updated))
        }
        Ok(false) => Err(StatusCode::CONFLICT),
        Err(sqlx::Error::Database(error)) if error.is_unique_violation() => {
            Err(StatusCode::CONFLICT)
        }
        Err(error) => {
            tracing::error!("update chat in SQLite failed: {}", error);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Delete a chat and all persisted messages. Private chats require their password.
#[utoipa::path(
    delete,
    path = "/api/chats/{id}",
    params(
        ("id" = Uuid, description = "Chat id"),
        ("x-room-password" = Option<String>, Header, description = "Required for private chats")
    ),
    responses(
        (status = 204, description = "Chat deleted"),
        (status = 401, description = "Incorrect chat password"),
        (status = 404, description = "Chat not found"),
        (status = 409, description = "Chat changed concurrently"),
        (status = 500, description = "Database error")
    )
)]
pub async fn delete_chat(
    State(state): State<SharedState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<StatusCode, StatusCode> {
    reject_private_chat(&state, id).await?;
    let chat = state.chat(id).await.ok_or(StatusCode::NOT_FOUND)?;
    let token = bearer_token(&headers)?;
    let user = state
        .session_user(token)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::UNAUTHORIZED)?;
    if !state
        .has_chat_permission(id, user.id, "room.delete")
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    {
        return Err(StatusCode::FORBIDDEN);
    }

    match state.delete_chat(id, &chat.password_hash).await {
        Ok(true) => Ok(StatusCode::NO_CONTENT),
        Ok(false) => Err(StatusCode::CONFLICT),
        Err(error) => {
            tracing::error!("delete chat from SQLite failed: {}", error);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}
