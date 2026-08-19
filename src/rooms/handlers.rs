//! Axum request handlers for the REST API.

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use chrono::Utc;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::message_store::MessageCursor;
use crate::models::{CreateRoomRequest, Room, StoredMessage, UpdateRoomRequest};
use crate::state::{with_pool, SharedState};
use crate::user_handlers::bearer_token;

const MAX_ROOM_NAME_CHARS: usize = 80;
const MAX_PASSWORD_CHARS: usize = 256;
const MAX_ROOM_AVATAR_CHARS: usize = 8;
const MAX_ROOM_DESCRIPTION_CHARS: usize = 300;

fn valid_room_avatar(value: &str) -> bool {
    value.chars().count() <= MAX_ROOM_AVATAR_CHARS && !value.chars().any(char::is_control)
}

fn valid_room_description(value: &str) -> bool {
    value.chars().count() <= MAX_ROOM_DESCRIPTION_CHARS && !value.chars().any(char::is_control)
}

fn hash_password(password: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(password.as_bytes());
    hex::encode(hasher.finalize())
}

fn valid_room_name(name: &str) -> bool {
    !name.is_empty() && name.chars().count() <= MAX_ROOM_NAME_CHARS
}

pub(crate) fn authorize_room(room: &Room, supplied: Option<&str>) -> bool {
    !room.has_password
        || supplied.is_some_and(|password| hash_password(password) == room.password_hash)
}

/// Create a chat room. Omit the password for a public room.
#[utoipa::path(
    post,
    path = "/api/rooms",
    request_body = CreateRoomRequest,
    responses(
        (status = 201, description = "Room created", body = Room),
        (status = 400, description = "Invalid room name or password"),
        (status = 409, description = "Room name already exists"),
        (status = 500, description = "Database error")
    )
)]
pub async fn create_room(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(req): Json<CreateRoomRequest>,
) -> Result<(StatusCode, Json<Room>), StatusCode> {
    let token = bearer_token(&headers)?;
    let creator = state
        .session_user(token)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::UNAUTHORIZED)?;
    let name = req.name.trim().to_string();
    if !valid_room_name(&name) {
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
    if !valid_room_avatar(&avatar_emoji) || !valid_room_description(&description) {
        return Err(StatusCode::BAD_REQUEST);
    }

    let id = Uuid::new_v4();
    let (password_hash, has_password) = match req.password.as_deref() {
        Some(password) if !password.is_empty() => (hash_password(password), true),
        _ => (String::new(), false),
    };

    let room = Room {
        id,
        name,
        password_hash,
        has_password,
        creator_user_id: Some(creator.id),
        join_policy: join_policy.to_string(),
        avatar_emoji,
        description,
        membership_status: Some("active".into()),
        membership_role: Some("owner".into()),
        unread_count: 0,
        created_at: Utc::now(),
    };

    match state.create_room_with_owner(room.clone(), creator.id).await {
        Ok(()) => Ok((StatusCode::CREATED, Json(room))),
        Err(sqlx::Error::Database(error)) if error.is_unique_violation() => {
            Err(StatusCode::CONFLICT)
        }
        Err(error) => {
            tracing::error!("create room in SQLite failed: {}", error);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Rename a room or change its password. Private rooms require the current password.
#[utoipa::path(
    patch,
    path = "/api/rooms/{id}",
    request_body = UpdateRoomRequest,
    responses(
        (status = 200, description = "Room updated", body = Room),
        (status = 400, description = "Invalid or empty update"),
        (status = 401, description = "Incorrect current password"),
        (status = 404, description = "Room not found"),
        (status = 409, description = "Name conflict or concurrent update"),
        (status = 500, description = "Database error")
    )
)]
pub async fn update_room(
    State(state): State<SharedState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(req): Json<UpdateRoomRequest>,
) -> Result<Json<Room>, StatusCode> {
    if state
        .is_direct_room(id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    {
        return Err(StatusCode::NOT_FOUND);
    }
    if req.name.is_none() && req.new_password.is_none() && req.join_policy.is_none() {
        return Err(StatusCode::BAD_REQUEST);
    }

    let token = bearer_token(&headers)?;
    let user = state
        .session_user(token)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::UNAUTHORIZED)?;
    if !state
        .has_room_permission(id, user.id, "room.settings")
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    {
        return Err(StatusCode::FORBIDDEN);
    }
    let previous = state.room(id).await.ok_or(StatusCode::NOT_FOUND)?;

    let name = req
        .name
        .as_deref()
        .map(str::trim)
        .unwrap_or(&previous.name)
        .to_string();
    if !valid_room_name(&name)
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
    if !valid_room_avatar(&avatar_emoji) || !valid_room_description(&description) {
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
    let updated = Room {
        id,
        name,
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
    };

    match state.update_room(&previous, updated.clone()).await {
        Ok(true) => {
            if password_changed {
                state
                    .restart_room_connections(id, "room password changed")
                    .await;
            } else if updated.name != previous.name {
                state
                    .broadcast(
                        id,
                        crate::models::ChatMessage::System {
                            content: format!("room renamed to {}", updated.name),
                            members: None,
                            participants: None,
                        },
                    )
                    .await;
            }
            Ok(Json(updated))
        }
        Ok(false) => Err(StatusCode::CONFLICT),
        Err(sqlx::Error::Database(error)) if error.is_unique_violation() => {
            Err(StatusCode::CONFLICT)
        }
        Err(error) => {
            tracing::error!("update room in SQLite failed: {}", error);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Delete a room and all persisted messages. Private rooms require their password.
#[utoipa::path(
    delete,
    path = "/api/rooms/{id}",
    params(
        ("id" = Uuid, description = "Room id"),
        ("x-room-password" = Option<String>, Header, description = "Required for private rooms")
    ),
    responses(
        (status = 204, description = "Room deleted"),
        (status = 401, description = "Incorrect room password"),
        (status = 404, description = "Room not found"),
        (status = 409, description = "Room changed concurrently"),
        (status = 500, description = "Database error")
    )
)]
pub async fn delete_room(
    State(state): State<SharedState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<StatusCode, StatusCode> {
    if state
        .is_direct_room(id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    {
        return Err(StatusCode::NOT_FOUND);
    }
    let room = state.room(id).await.ok_or(StatusCode::NOT_FOUND)?;
    let token = bearer_token(&headers)?;
    let user = state
        .session_user(token)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::UNAUTHORIZED)?;
    if !state
        .has_room_permission(id, user.id, "room.delete")
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    {
        return Err(StatusCode::FORBIDDEN);
    }

    match state.delete_room(id, &room.password_hash).await {
        Ok(true) => Ok(StatusCode::NO_CONTENT),
        Ok(false) => Err(StatusCode::CONFLICT),
        Err(error) => {
            tracing::error!("delete room from SQLite failed: {}", error);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

#[derive(Deserialize)]
pub struct MessageHistoryQuery {
    pub limit: Option<i64>,
    /// Exclusive cursor: return messages strictly older than this message id.
    pub before: Option<Uuid>,
}

/// Return persisted room messages in reverse-chronological pages (newest first,
/// or strictly older than `before`) for history backfill.
#[utoipa::path(
    get,
    path = "/api/rooms/{id}/messages",
    params(
        ("id" = Uuid, description = "Room id"),
        ("limit" = Option<i64>, Query, description = "Messages to return (1-500)"),
        ("before" = Option<Uuid>, Query, description = "Exclusive message cursor for backfilling older history"),
        ("x-room-password" = Option<String>, Header, description = "Required for private rooms")
    ),
    responses(
        (status = 200, description = "Persisted room messages", body = Vec<StoredMessage>),
        (status = 400, description = "Unknown `before` cursor"),
        (status = 401, description = "Missing or incorrect room password"),
        (status = 404, description = "Room not found"),
        (status = 500, description = "Database error")
    )
)]
pub async fn list_messages(
    State(state): State<SharedState>,
    Path(id): Path<Uuid>,
    Query(query): Query<MessageHistoryQuery>,
    headers: HeaderMap,
) -> Result<Json<Vec<StoredMessage>>, StatusCode> {
    let room = state.room(id).await.ok_or(StatusCode::NOT_FOUND)?;
    let token = bearer_token(&headers)?;
    let user = state
        .session_user(token)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::UNAUTHORIZED)?;
    if !state
        .has_room_permission(id, user.id, "message.send")
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    {
        return Err(StatusCode::FORBIDDEN);
    }
    if room.has_password {
        let supplied = headers
            .get("x-room-password")
            .and_then(|value| value.to_str().ok());
        let Some(supplied) = supplied else {
            return Err(StatusCode::UNAUTHORIZED);
        };
        if !authorize_room(&room, Some(supplied)) {
            return Err(StatusCode::UNAUTHORIZED);
        }
    }

    let before = match query.before {
        Some(message_id) => {
            let created_at = with_pool!(state, |pool| {
                sqlx::query_scalar::<_, chrono::DateTime<Utc>>(
                    "SELECT created_at FROM messages WHERE id = $1 AND room_id = $2",
                )
                .bind(message_id)
                .bind(id)
                .fetch_optional(pool)
                .await
            })
            .map_err(|error| {
                tracing::error!("resolve message cursor failed: {}", error);
                StatusCode::INTERNAL_SERVER_ERROR
            })?
            .ok_or(StatusCode::BAD_REQUEST)?;
            Some(MessageCursor {
                created_at,
                id: message_id,
            })
        }
        None => None,
    };

    match state
        .message_history(
            id,
            query.limit.unwrap_or(100),
            before.as_ref(),
            Some(user.id),
        )
        .await
    {
        Ok(messages) => Ok(Json(messages)),
        Err(error) => {
            tracing::error!("load room message history failed: {}", error);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}
