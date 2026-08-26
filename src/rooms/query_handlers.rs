//! Viewer-aware room discovery and detail queries.

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::models::Room;
use crate::state::SharedState;
use crate::user_handlers::optional_bearer_token;

#[derive(Deserialize)]
pub struct ListQuery {
    pub name: Option<String>,
}

#[utoipa::path(
    get,
    path = "/api/rooms",
    params(("name" = Option<String>, Query, description = "Filter by exact room name")),
    responses((status = 200, description = "Matching rooms", body = Vec<Room>))
)]
pub async fn list_rooms(
    State(state): State<SharedState>,
    Query(query): Query<ListQuery>,
    headers: HeaderMap,
) -> Result<Json<Vec<Room>>, StatusCode> {
    let direct_ids = state
        .direct_room_ids()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let mut rooms = state.list_rooms(query.name.as_deref()).await;
    rooms.retain(|room| !direct_ids.contains(&room.id));
    let user = if let Some(token) = optional_bearer_token(&headers) {
        state
            .session_user(token)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    } else {
        None
    };
    let Some(user) = user else {
        return Ok(Json(Vec::new()));
    };
    state
        .decorate_rooms_for_user(&mut rooms, user.id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    rooms.retain(|room| room.membership_status.as_deref() == Some("active"));
    Ok(Json(rooms))
}

#[utoipa::path(
    get,
    path = "/api/rooms/discover",
    params(("name" = Option<String>, Query, description = "Filter by exact room name")),
    responses((status = 200, description = "Discoverable public rooms", body = Vec<Room>))
)]
pub async fn discover_rooms(
    State(state): State<SharedState>,
    Query(query): Query<ListQuery>,
    headers: HeaderMap,
) -> Result<Json<Vec<Room>>, StatusCode> {
    let direct_ids = state
        .direct_room_ids()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let mut rooms = state.list_rooms(query.name.as_deref()).await;
    rooms.retain(|room| !direct_ids.contains(&room.id) && !room.has_password);

    if let Some(token) = optional_bearer_token(&headers) {
        if let Some(user) = state
            .session_user(token)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        {
            state
                .decorate_rooms_for_user(&mut rooms, user.id)
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            rooms.retain(|room| room.membership_status.as_deref() != Some("active"));
        }
    }
    Ok(Json(rooms))
}

#[utoipa::path(
    get,
    path = "/api/rooms/{id}",
    params(("id" = Uuid, description = "Room id")),
    responses(
        (status = 200, description = "Room found", body = Room),
        (status = 404, description = "Room not found")
    )
)]
pub async fn get_room(
    State(state): State<SharedState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Room>, StatusCode> {
    let mut room = state.room(id).await.ok_or(StatusCode::NOT_FOUND)?;
    room.membership_status = None;
    room.membership_role = None;
    let user = if let Some(token) = optional_bearer_token(&headers) {
        state
            .session_user(token)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    } else {
        None
    };
    if let Some(user) = &user {
        if let Some((status, role)) = state
            .membership_identity(id, user.id)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        {
            room.membership_status = Some(status);
            room.membership_role = Some(role);
        }
    }
    let direct = state
        .is_direct_room(id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if direct {
        if room.membership_status.as_deref() != Some("active") {
            return Err(StatusCode::NOT_FOUND);
        }
        let user = user.ok_or(StatusCode::NOT_FOUND)?;
        let conversation = state
            .conversation_summary(user.id, id)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
            .ok_or(StatusCode::NOT_FOUND)?;
        room.name = conversation.title;
        room.avatar_emoji = conversation.avatar_emoji;
        room.description = conversation.description;
    }
    Ok(Json(room))
}
