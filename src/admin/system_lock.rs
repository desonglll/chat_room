//! Persistent administrator-controlled lock for every chat room.

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
pub const ROOM_LOCK_REASON: &str = "room locked";

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateSystemLockRequest {
    locked: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SystemLockStatus {
    pub locked: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct RoomLockStatus {
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

    pub async fn room_locked(&self, room_id: Uuid) -> Result<bool, sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query_scalar::<_, bool>(
                "SELECT EXISTS(SELECT 1 FROM rooms \
                 WHERE id = $1 AND deleted_at IS NULL AND locked_at IS NOT NULL)",
            )
            .bind(room_id)
            .fetch_one(pool)
            .await
        })
    }

    async fn set_room_locked(&self, room_id: Uuid, locked: bool) -> Result<bool, sqlx::Error> {
        let locked_at = locked.then(Utc::now);
        with_pool!(self, |pool| {
            sqlx::query("UPDATE rooms SET locked_at = $1 WHERE id = $2 AND deleted_at IS NULL")
                .bind(locked_at)
                .bind(room_id)
                .execute(pool)
                .await
                .map(|result| result.rows_affected() > 0)
        })
    }
}

pub(crate) async fn room_lock_reason(
    state: &AppState,
    room_id: Uuid,
) -> Result<Option<&'static str>, sqlx::Error> {
    if state.chat_rooms_locked().await? {
        return Ok(Some(SYSTEM_LOCK_REASON));
    }
    Ok(state
        .room_locked(room_id)
        .await?
        .then_some(ROOM_LOCK_REASON))
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

pub(crate) async fn require_room_unlocked(
    state: &AppState,
    room_id: Uuid,
) -> Result<(), StatusCode> {
    match room_lock_reason(state, room_id).await {
        Ok(None) => Ok(()),
        Ok(Some(_)) => Err(StatusCode::LOCKED),
        Err(error) => {
            tracing::error!("read room chat lock failed: {error}");
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/admin/room-locks/{room_id}",
    params(("room_id" = Uuid, Path, description = "Room identifier")),
    responses(
        (status = 200, description = "Current room lock", body = RoomLockStatus),
        (status = 401, description = "Missing or expired session"),
        (status = 403, description = "Account is not a system administrator"),
        (status = 404, description = "Room does not exist")
    )
)]
pub async fn room_status(
    State(state): State<SharedState>,
    Path(room_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<RoomLockStatus>, StatusCode> {
    require_admin(&state, &headers).await?;
    state.room(room_id).await.ok_or(StatusCode::NOT_FOUND)?;
    let locked = state.room_locked(room_id).await.map_err(|error| {
        tracing::error!("read room chat lock failed: {error}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    Ok(Json(RoomLockStatus { room_id, locked }))
}

#[utoipa::path(
    put,
    path = "/api/admin/room-locks/{room_id}",
    params(("room_id" = Uuid, Path, description = "Room identifier")),
    request_body = UpdateSystemLockRequest,
    responses(
        (status = 200, description = "Updated room lock", body = RoomLockStatus),
        (status = 401, description = "Missing or expired session"),
        (status = 403, description = "Account is not a system administrator"),
        (status = 404, description = "Room does not exist")
    )
)]
pub async fn update_room(
    State(state): State<SharedState>,
    Path(room_id): Path<Uuid>,
    headers: HeaderMap,
    Json(request): Json<UpdateSystemLockRequest>,
) -> Result<Json<RoomLockStatus>, StatusCode> {
    let actor = require_admin(&state, &headers).await?;
    state
        .record_audit_event(
            AuditEventDraft::system(&actor, "room.lock.update_requested")
                .target("room", room_id)
                .detail("locked", request.locked),
        )
        .await
        .map_err(|error| {
            tracing::error!("required Room lock audit failed: {error}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    let updated = state
        .set_room_locked(room_id, request.locked)
        .await
        .map_err(|error| {
            tracing::error!("update room chat lock failed: {error}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    if !updated {
        return Err(StatusCode::NOT_FOUND);
    }
    if request.locked {
        state
            .restart_room_connections(room_id, ROOM_LOCK_REASON)
            .await;
    }
    Ok(Json(RoomLockStatus {
        room_id,
        locked: request.locked,
    }))
}

#[utoipa::path(
    put,
    path = "/api/admin/chat-lock",
    request_body = UpdateSystemLockRequest,
    responses(
        (status = 200, description = "Updated global chat room lock", body = SystemLockStatus),
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
