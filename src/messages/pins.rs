//! Conversation-scoped pinned messages and favorite-document sharing.

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::models::{StoredMessage, User};
use crate::state::{with_pool, AppState, SharedState};
use crate::user_handlers::bearer_token;

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct RoomPin {
    pub message: StoredMessage,
    pub pinned_by: Uuid,
    pub pinned_at: DateTime<Utc>,
}

impl AppState {
    pub async fn room_pins(
        &self,
        room_id: Uuid,
        viewer_id: Uuid,
    ) -> Result<Vec<RoomPin>, sqlx::Error> {
        let rows: Vec<(Uuid, Uuid, DateTime<Utc>)> = with_pool!(self, |pool| {
            sqlx::query_as(
                "SELECT message_id, pinned_by, pinned_at FROM room_pins \
                 WHERE room_id = $1 ORDER BY pinned_at DESC, message_id LIMIT 100",
            )
            .bind(room_id)
            .fetch_all(pool)
            .await
        })?;
        let mut pins = Vec::with_capacity(rows.len());
        for (message_id, pinned_by, pinned_at) in rows {
            if let Some(message) = self.message_by_id(message_id, Some(viewer_id)).await? {
                pins.push(RoomPin {
                    message,
                    pinned_by,
                    pinned_at,
                });
            }
        }
        Ok(pins)
    }

    pub async fn pin_room_message(
        &self,
        room_id: Uuid,
        message_id: Uuid,
        user_id: Uuid,
    ) -> Result<Option<RoomPin>, sqlx::Error> {
        let pinned_at = Utc::now();
        let inserted = with_pool!(self, |pool| {
            sqlx::query(
                "INSERT INTO room_pins (room_id, message_id, pinned_by, pinned_at) \
                 SELECT $1, messages.id, $3, $4 FROM messages JOIN rooms ON rooms.id = messages.room_id \
                 WHERE messages.id = $2 AND messages.room_id = $1 \
                   AND messages.recalled_at IS NULL AND rooms.deleted_at IS NULL \
                 ON CONFLICT (room_id, message_id) DO UPDATE SET \
                   pinned_by = excluded.pinned_by, pinned_at = excluded.pinned_at",
            )
            .bind(room_id)
            .bind(message_id)
            .bind(user_id)
            .bind(pinned_at)
            .execute(pool)
            .await
            .map(|result| result.rows_affected() > 0)
        })?;
        if !inserted {
            return Ok(None);
        }
        Ok(self
            .message_by_id(message_id, Some(user_id))
            .await?
            .map(|message| RoomPin {
                message,
                pinned_by: user_id,
                pinned_at,
            }))
    }

    pub async fn unpin_room_message(
        &self,
        room_id: Uuid,
        message_id: Uuid,
    ) -> Result<bool, sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query("DELETE FROM room_pins WHERE room_id = $1 AND message_id = $2")
                .bind(room_id)
                .bind(message_id)
                .execute(pool)
                .await
                .map(|result| result.rows_affected() > 0)
        })
    }
}

async fn current_user(state: &SharedState, headers: &HeaderMap) -> Result<User, StatusCode> {
    state
        .session_user(bearer_token(headers)?)
        .await
        .map_err(internal_error)?
        .ok_or(StatusCode::UNAUTHORIZED)
}

async fn require_active_member(
    state: &SharedState,
    room_id: Uuid,
    user_id: Uuid,
) -> Result<(), StatusCode> {
    state
        .membership_identity(room_id, user_id)
        .await
        .map_err(internal_error)?
        .is_some_and(|(status, _)| status == "active")
        .then_some(())
        .ok_or(StatusCode::FORBIDDEN)
}

async fn require_pin_permission(
    state: &SharedState,
    room_id: Uuid,
    user_id: Uuid,
) -> Result<(), StatusCode> {
    if state
        .is_direct_room(room_id)
        .await
        .map_err(internal_error)?
    {
        return require_active_member(state, room_id, user_id).await;
    }
    state
        .has_room_permission(room_id, user_id, "message.pin")
        .await
        .map_err(internal_error)?
        .then_some(())
        .ok_or(StatusCode::FORBIDDEN)
}

#[utoipa::path(get, path = "/api/rooms/{room_id}/pins", responses((status = 200, body = [RoomPin])))]
pub async fn list_pins(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(room_id): Path<Uuid>,
) -> Result<Json<Vec<RoomPin>>, StatusCode> {
    let user = current_user(&state, &headers).await?;
    require_active_member(&state, room_id, user.id).await?;
    state
        .room_pins(room_id, user.id)
        .await
        .map(Json)
        .map_err(internal_error)
}

#[utoipa::path(post, path = "/api/rooms/{room_id}/pins/{message_id}", responses((status = 201, body = RoomPin)))]
pub async fn pin_message(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path((room_id, message_id)): Path<(Uuid, Uuid)>,
) -> Result<(StatusCode, Json<RoomPin>), StatusCode> {
    let user = current_user(&state, &headers).await?;
    require_pin_permission(&state, room_id, user.id).await?;
    state
        .pin_room_message(room_id, message_id, user.id)
        .await
        .map_err(internal_error)?
        .map(|pin| (StatusCode::CREATED, Json(pin)))
        .ok_or(StatusCode::NOT_FOUND)
}

#[utoipa::path(delete, path = "/api/rooms/{room_id}/pins/{message_id}", responses((status = 204)))]
pub async fn unpin_message(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path((room_id, message_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, StatusCode> {
    let user = current_user(&state, &headers).await?;
    require_pin_permission(&state, room_id, user.id).await?;
    state
        .unpin_room_message(room_id, message_id)
        .await
        .map_err(internal_error)?
        .then_some(StatusCode::NO_CONTENT)
        .ok_or(StatusCode::NOT_FOUND)
}

fn internal_error(error: sqlx::Error) -> StatusCode {
    tracing::error!("room pin operation failed: {error}");
    StatusCode::INTERNAL_SERVER_ERROR
}
