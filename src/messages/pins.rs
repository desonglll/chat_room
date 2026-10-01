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

use crate::models::{ChatMessage, StoredMessage, User};
use crate::state::{with_pool, AppState, SharedState};
use crate::user_handlers::bearer_token;

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ChatPin {
    pub message: StoredMessage,
    pub pinned_by: Uuid,
    pub pinned_at: DateTime<Utc>,
}

impl AppState {
    pub async fn chat_pins(
        &self,
        room_id: Uuid,
        viewer_id: Uuid,
    ) -> Result<Vec<ChatPin>, sqlx::Error> {
        let rows: Vec<(Uuid, Uuid, DateTime<Utc>)> = with_pool!(self, |pool| {
            sqlx::query_as(
                "SELECT message_id, pinned_by, pinned_at FROM chat_pins \
                 WHERE room_id = $1 ORDER BY pinned_at DESC, message_id LIMIT 100",
            )
            .bind(room_id)
            .fetch_all(pool)
            .await
        })?;
        let mut pins = Vec::with_capacity(rows.len());
        for (message_id, pinned_by, pinned_at) in rows {
            if let Some(message) = self.message_by_id(message_id, Some(viewer_id)).await? {
                pins.push(ChatPin {
                    message,
                    pinned_by,
                    pinned_at,
                });
            }
        }
        Ok(pins)
    }

    pub async fn pin_chat_message(
        &self,
        room_id: Uuid,
        message_id: Uuid,
        user_id: Uuid,
    ) -> Result<Option<ChatPin>, sqlx::Error> {
        let pinned_at = Utc::now();
        let inserted = with_pool!(self, |pool| {
            sqlx::query(
                "INSERT INTO chat_pins (room_id, message_id, pinned_by, pinned_at) \
                 SELECT $1, messages.id, $3, $4 FROM messages JOIN chats ON chats.id = messages.room_id \
                 WHERE messages.id = $2 AND messages.room_id = $1 \
                   AND messages.recalled_at IS NULL AND chats.deleted_at IS NULL \
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
            .map(|message| ChatPin {
                message,
                pinned_by: user_id,
                pinned_at,
            }))
    }

    pub async fn unpin_chat_message(
        &self,
        room_id: Uuid,
        message_id: Uuid,
    ) -> Result<bool, sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query("DELETE FROM chat_pins WHERE room_id = $1 AND message_id = $2")
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
    if state.is_private_chat(room_id).await {
        return require_active_member(state, room_id, user_id).await;
    }
    state
        .has_chat_permission(room_id, user_id, "message.pin")
        .await
        .map_err(internal_error)?
        .then_some(())
        .ok_or(StatusCode::FORBIDDEN)
}

#[utoipa::path(get, path = "/api/chats/{room_id}/pins", responses((status = 200, body = [ChatPin])))]
pub async fn list_pins(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(room_id): Path<Uuid>,
) -> Result<Json<Vec<ChatPin>>, StatusCode> {
    let user = current_user(&state, &headers).await?;
    require_active_member(&state, room_id, user.id).await?;
    state
        .chat_pins(room_id, user.id)
        .await
        .map(Json)
        .map_err(internal_error)
}

#[utoipa::path(post, path = "/api/chats/{room_id}/pins/{message_id}", responses((status = 201, body = ChatPin)))]
pub async fn pin_message(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path((room_id, message_id)): Path<(Uuid, Uuid)>,
) -> Result<(StatusCode, Json<ChatPin>), StatusCode> {
    let user = current_user(&state, &headers).await?;
    require_pin_permission(&state, room_id, user.id).await?;
    let pin = state
        .pin_chat_message(room_id, message_id, user.id)
        .await
        .map_err(internal_error)?
        .ok_or(StatusCode::NOT_FOUND)?;
    // TG-901: every open client refreshes its pinned bar.
    state
        .broadcast(
            room_id,
            ChatMessage::PinsChanged {
                message_id,
                pinned: true,
            },
        )
        .await;
    Ok((StatusCode::CREATED, Json(pin)))
}

#[utoipa::path(delete, path = "/api/chats/{room_id}/pins/{message_id}", responses((status = 204)))]
pub async fn unpin_message(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path((room_id, message_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, StatusCode> {
    let user = current_user(&state, &headers).await?;
    require_pin_permission(&state, room_id, user.id).await?;
    let removed = state
        .unpin_chat_message(room_id, message_id)
        .await
        .map_err(internal_error)?;
    if !removed {
        return Err(StatusCode::NOT_FOUND);
    }
    state
        .broadcast(
            room_id,
            ChatMessage::PinsChanged {
                message_id,
                pinned: false,
            },
        )
        .await;
    Ok(StatusCode::NO_CONTENT)
}

fn internal_error(error: sqlx::Error) -> StatusCode {
    tracing::error!("chat pin operation failed: {error}");
    StatusCode::INTERNAL_SERVER_ERROR
}
