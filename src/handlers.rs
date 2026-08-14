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

use crate::models::{CreateRoomRequest, Room, StoredMessage, UpdateRoomRequest};
use crate::state::SharedState;

const MAX_ROOM_NAME_CHARS: usize = 80;
const MAX_PASSWORD_CHARS: usize = 256;

fn hash_password(password: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(password.as_bytes());
    hex::encode(hasher.finalize())
}

fn valid_room_name(name: &str) -> bool {
    !name.is_empty() && name.chars().count() <= MAX_ROOM_NAME_CHARS
}

fn authorize_room(room: &Room, supplied: Option<&str>) -> bool {
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
    Json(req): Json<CreateRoomRequest>,
) -> Result<(StatusCode, Json<Room>), StatusCode> {
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
        created_at: Utc::now(),
    };

    match state.insert_room(room.clone()).await {
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

/// Query-string wrapper for name filtering.
#[derive(Deserialize)]
pub struct ListQuery {
    pub name: Option<String>,
}

/// List rooms. Pass ?name=... to filter by exact room name.
#[utoipa::path(
    get,
    path = "/api/rooms",
    params(
        ("name" = Option<String>, Query, description = "Filter by exact room name")
    ),
    responses(
        (status = 200, description = "Matching rooms", body = Vec<Room>)
    )
)]
pub async fn list_rooms(
    State(state): State<SharedState>,
    Query(query): Query<ListQuery>,
) -> Json<Vec<Room>> {
    Json(state.list_rooms(query.name.as_deref()).await)
}

/// Fetch a single room by UUID.
#[utoipa::path(
    get,
    path = "/api/rooms/{id}",
    params(
        ("id" = Uuid, description = "Room id")
    ),
    responses(
        (status = 200, description = "Room found", body = Room),
        (status = 404, description = "Room not found")
    )
)]
pub async fn get_room(
    State(state): State<SharedState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Room>, StatusCode> {
    state.room(id).await.map(Json).ok_or(StatusCode::NOT_FOUND)
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
    Json(req): Json<UpdateRoomRequest>,
) -> Result<Json<Room>, StatusCode> {
    if req.name.is_none() && req.new_password.is_none() {
        return Err(StatusCode::BAD_REQUEST);
    }

    let previous = state.room(id).await.ok_or(StatusCode::NOT_FOUND)?;
    if !authorize_room(&previous, req.current_password.as_deref()) {
        return Err(StatusCode::UNAUTHORIZED);
    }

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
    let room = state.room(id).await.ok_or(StatusCode::NOT_FOUND)?;
    let supplied = headers
        .get("x-room-password")
        .and_then(|value| value.to_str().ok());
    if !authorize_room(&room, supplied) {
        return Err(StatusCode::UNAUTHORIZED);
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
}

/// Return persisted room messages in chronological order.
#[utoipa::path(
    get,
    path = "/api/rooms/{id}/messages",
    params(
        ("id" = Uuid, description = "Room id"),
        ("limit" = Option<i64>, Query, description = "Newest messages to return (1-500)"),
        ("x-room-password" = Option<String>, Header, description = "Required for private rooms")
    ),
    responses(
        (status = 200, description = "Persisted room messages", body = Vec<StoredMessage>),
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

    match state
        .message_history(id, query.limit.unwrap_or(100), None)
        .await
    {
        Ok(messages) => Ok(Json(messages)),
        Err(error) => {
            tracing::error!("load room message history failed: {}", error);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}
