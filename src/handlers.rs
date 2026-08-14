//! Axum request handlers for the REST API.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use chrono::Utc;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::models::{CreateRoomRequest, Room};
use crate::state::SharedState;

/// Create a chat room. Omit the password for a public room.
#[utoipa::path(
    post,
    path = "/api/rooms",
    request_body = CreateRoomRequest,
    responses(
        (status = 201, description = "Room created", body = Room),
        (status = 409, description = "Room name already exists"),
        (status = 500, description = "Database error")
    )
)]
pub async fn create_room(
    State(state): State<SharedState>,
    Json(req): Json<CreateRoomRequest>,
) -> Result<(StatusCode, Json<Room>), StatusCode> {
    let id = Uuid::new_v4();
    let (password_hash, has_password) = match req.password.as_deref() {
        Some(password) if !password.is_empty() => {
            let mut hasher = Sha256::new();
            hasher.update(password.as_bytes());
            (hex::encode(hasher.finalize()), true)
        }
        _ => (String::new(), false),
    };

    let room = Room {
        id,
        name: req.name,
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
