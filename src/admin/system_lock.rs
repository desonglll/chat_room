//! Persistent administrator-controlled lock for every chat.

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use super::access::require_admin;
use crate::audit::AuditEventDraft;
use crate::state::{with_pool, AppState, SharedState};

pub const SYSTEM_LOCK_REASON: &str = "system locked";
pub const CHAT_LOCK_REASON: &str = "chat locked";

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateSystemLockRequest {
    locked: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SystemLockStatus {
    pub locked: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ChatLockStatus {
    pub room_id: Uuid,
    pub locked: bool,
}

impl AppState {
    pub async fn chat_rooms_locked(&self) -> Result<bool, sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query_scalar::<_, String>(
                "SELECT value FROM system_settings WHERE key = 'chat_rooms_locked'",
            )
            .fetch_one(pool)
            .await
            .map(|value| value == "true")
        })
    }

    pub(crate) async fn set_chat_rooms_locked(&self, locked: bool) -> Result<(), sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query("UPDATE system_settings SET value = $1 WHERE key = 'chat_rooms_locked'")
                .bind(if locked { "true" } else { "false" })
                .execute(pool)
                .await
                .map(|_| ())
        })
    }

    pub async fn chat_locked(&self, room_id: Uuid) -> Result<bool, sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query_scalar::<_, bool>(
                "SELECT EXISTS(SELECT 1 FROM chats \
                 WHERE id = $1 AND deleted_at IS NULL AND locked_at IS NOT NULL)",
            )
            .bind(room_id)
            .fetch_one(pool)
            .await
        })
    }

    async fn set_chat_locked(&self, room_id: Uuid, locked: bool) -> Result<bool, sqlx::Error> {
        let locked_at = locked.then(Utc::now);
        with_pool!(self, |pool| {
            sqlx::query("UPDATE chats SET locked_at = $1 WHERE id = $2 AND deleted_at IS NULL")
                .bind(locked_at)
                .bind(room_id)
                .execute(pool)
                .await
                .map(|result| result.rows_affected() > 0)
        })
    }
}

pub(crate) async fn chat_lock_reason(
    state: &AppState,
    room_id: Uuid,
) -> Result<Option<&'static str>, sqlx::Error> {
    if state.chat_rooms_locked().await? {
        return Ok(Some(SYSTEM_LOCK_REASON));
    }
    Ok(state
        .chat_locked(room_id)
        .await?
        .then_some(CHAT_LOCK_REASON))
}

pub(crate) async fn require_chat_rooms_unlocked(state: &AppState) -> Result<(), StatusCode> {
    match state.chat_rooms_locked().await {
        Ok(false) => Ok(()),
        Ok(true) => Err(StatusCode::LOCKED),
        Err(error) => {
            tracing::error!("read system chat lock failed: {error}");
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub(crate) async fn require_chat_unlocked(
    state: &AppState,
    room_id: Uuid,
) -> Result<(), StatusCode> {
    match chat_lock_reason(state, room_id).await {
        Ok(None) => Ok(()),
        Ok(Some(_)) => Err(StatusCode::LOCKED),
        Err(error) => {
            tracing::error!("read chat lock failed: {error}");
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/admin/room-locks/{room_id}",
    params(("room_id" = Uuid, Path, description = "Chat identifier")),
    responses(
        (status = 200, description = "Current chat lock", body = ChatLockStatus),
        (status = 401, description = "Missing or expired session"),
        (status = 403, description = "Account is not a system administrator"),
        (status = 404, description = "Chat does not exist")
    )
)]
pub async fn chat_status(
    State(state): State<SharedState>,
    Path(room_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<ChatLockStatus>, StatusCode> {
    require_admin(&state, &headers).await?;
    state.chat(room_id).await.ok_or(StatusCode::NOT_FOUND)?;
    let locked = state.chat_locked(room_id).await.map_err(|error| {
        tracing::error!("read chat lock failed: {error}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    Ok(Json(ChatLockStatus { room_id, locked }))
}

#[utoipa::path(
    put,
    path = "/api/admin/room-locks/{room_id}",
    params(("room_id" = Uuid, Path, description = "Chat identifier")),
    request_body = UpdateSystemLockRequest,
    responses(
        (status = 200, description = "Updated chat lock", body = ChatLockStatus),
        (status = 401, description = "Missing or expired session"),
        (status = 403, description = "Account is not a system administrator"),
        (status = 404, description = "Chat does not exist")
    )
)]
pub async fn update_chat(
    State(state): State<SharedState>,
    Path(room_id): Path<Uuid>,
    headers: HeaderMap,
    Json(request): Json<UpdateSystemLockRequest>,
) -> Result<Json<ChatLockStatus>, StatusCode> {
    let actor = require_admin(&state, &headers).await?;
    state
        .record_audit_event(
            AuditEventDraft::system(&actor, "room.lock.update_requested")
                // `room` is the frozen audit `target_type` value; web/src/auditApi.ts types it.
                .target("room", room_id)
                .detail("locked", request.locked),
        )
        .await
        .map_err(|error| {
            tracing::error!("required Chat lock audit failed: {error}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    let updated = state
        .set_chat_locked(room_id, request.locked)
        .await
        .map_err(|error| {
            tracing::error!("update chat lock failed: {error}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    if !updated {
        return Err(StatusCode::NOT_FOUND);
    }
    if request.locked {
        state
            .restart_chat_connections(room_id, CHAT_LOCK_REASON)
            .await;
    }
    Ok(Json(ChatLockStatus {
        room_id,
        locked: request.locked,
    }))
}

#[utoipa::path(
    put,
    path = "/api/admin/chat-lock",
    request_body = UpdateSystemLockRequest,
    responses(
        (status = 200, description = "Updated global chat lock", body = SystemLockStatus),
        (status = 401, description = "Missing or expired session"),
        (status = 403, description = "Account is not a system administrator")
    )
)]
pub async fn update(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(request): Json<UpdateSystemLockRequest>,
) -> Result<Json<SystemLockStatus>, StatusCode> {
    let actor = require_admin(&state, &headers).await?;
    state
        .record_audit_event(
            AuditEventDraft::system(&actor, "system.lock.update_requested")
                .target_type("system")
                .detail("locked", request.locked),
        )
        .await
        .map_err(|error| {
            tracing::error!("required system lock audit failed: {error}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    state
        .set_chat_rooms_locked(request.locked)
        .await
        .map_err(|error| {
            tracing::error!("update system chat lock failed: {error}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    if request.locked {
        state.disconnect_all_chat_rooms(SYSTEM_LOCK_REASON).await;
    }
    Ok(Json(SystemLockStatus {
        locked: request.locked,
    }))
}
