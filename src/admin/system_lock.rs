//! Persistent administrator-controlled lock for every chat room.

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    Json,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::metrics::require_admin;
use crate::state::{with_pool, AppState, SharedState};

pub const SYSTEM_LOCK_REASON: &str = "system locked";

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateSystemLockRequest {
    locked: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SystemLockStatus {
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

    async fn set_chat_rooms_locked(&self, locked: bool) -> Result<(), sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query("UPDATE system_settings SET value = $1 WHERE key = 'chat_rooms_locked'")
                .bind(if locked { "true" } else { "false" })
                .execute(pool)
                .await
                .map(|_| ())
        })
    }
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
    require_admin(&state, &headers).await?;
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
