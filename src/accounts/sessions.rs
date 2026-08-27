//! Privacy-safe device session metadata and account-facing session management.

use std::net::{IpAddr, SocketAddr};

use axum::{
    extract::{ConnectInfo, Path, State},
    http::{HeaderMap, StatusCode},
    routing::{delete, get},
    Json, Router,
};
use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    security::client_address,
    state::{with_pool, AppState, SharedState},
};

use super::user_handlers::bearer_token;

pub(crate) fn routes() -> Router<SharedState> {
    Router::new()
        .route("/api/users/me/sessions", get(list))
        .route("/api/users/me/sessions/others", delete(revoke_others))
        .route("/api/users/me/sessions/:id", delete(revoke))
}

#[derive(Clone, Debug)]
pub(crate) struct SessionMetadata {
    pub device_name: String,
    pub ip_hint: String,
}

impl Default for SessionMetadata {
    fn default() -> Self {
        Self {
            device_name: "Unknown device".into(),
            ip_hint: String::new(),
        }
    }
}

impl SessionMetadata {
    pub(crate) fn from_request(
        headers: &HeaderMap,
        peer: Option<ConnectInfo<SocketAddr>>,
        trust_proxy_headers: bool,
    ) -> Self {
        let user_agent = headers
            .get(axum::http::header::USER_AGENT)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default();
        let address = client_address(headers, peer, trust_proxy_headers);
        Self {
            device_name: device_name(user_agent),
            ip_hint: coarse_ip(&address).unwrap_or_default(),
        }
    }
}

#[derive(Debug, Serialize, ToSchema, sqlx::FromRow)]
pub struct DeviceSession {
    /// Opaque management identifier. This is never the bearer session token.
    pub id: String,
    pub device_name: String,
    pub ip_hint: Option<String>,
    pub created_at: DateTime<Utc>,
    pub last_used_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub current: bool,
}

pub(crate) enum RevokeSessionResult {
    Revoked,
    Current,
    NotFound,
}

impl AppState {
    pub async fn device_sessions(
        &self,
        user_id: Uuid,
        current_session: Uuid,
    ) -> Result<Vec<DeviceSession>, sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query_as(
                "SELECT management_id AS id, device_name, NULLIF(ip_hint, '') AS ip_hint, \
                 created_at, last_used_at, expires_at, \
                 CASE WHEN id = $2 THEN TRUE ELSE FALSE END AS current \
                 FROM sessions WHERE user_id = $1 AND expires_at > $3 \
                 AND management_id IS NOT NULL ORDER BY last_used_at DESC, id",
            )
            .bind(user_id)
            .bind(current_session)
            .bind(Utc::now())
            .fetch_all(pool)
            .await
        })
    }

    pub(crate) async fn revoke_device_session(
        &self,
        user_id: Uuid,
        current_session: Uuid,
        management_id: &str,
    ) -> Result<RevokeSessionResult, sqlx::Error> {
        let revoked: Option<Uuid> = with_pool!(self, |pool| {
            sqlx::query_scalar(
                "DELETE FROM sessions WHERE user_id = $1 AND management_id = $2 AND id <> $3 \
                 RETURNING id",
            )
            .bind(user_id)
            .bind(management_id)
            .bind(current_session)
            .fetch_optional(pool)
            .await
        })?;
        if let Some(token) = revoked {
            if let Some(cache) = self.redis_cache() {
                if let Err(error) = cache.delete_session(token, user_id).await {
                    tracing::warn!("delete revoked session from Redis failed: {error:#}");
                }
            }
            return Ok(RevokeSessionResult::Revoked);
        }
        let is_current: bool = with_pool!(self, |pool| {
            sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM sessions WHERE user_id = $1 \
                 AND management_id = $2 AND id = $3)",
            )
            .bind(user_id)
            .bind(management_id)
            .bind(current_session)
            .fetch_one(pool)
            .await
        })?;
        Ok(if is_current {
            RevokeSessionResult::Current
        } else {
            RevokeSessionResult::NotFound
        })
    }

    pub(crate) async fn revoke_other_device_sessions(
        &self,
        user_id: Uuid,
        current_session: Uuid,
    ) -> Result<usize, sqlx::Error> {
        let revoked: Vec<Uuid> = with_pool!(self, |pool| {
            sqlx::query_scalar("DELETE FROM sessions WHERE user_id = $1 AND id <> $2 RETURNING id")
                .bind(user_id)
                .bind(current_session)
                .fetch_all(pool)
                .await
        })?;
        if !revoked.is_empty() {
            self.invalidate_user_sessions(user_id).await;
        }
        Ok(revoked.len())
    }
}

/// List the current account's active device sessions.
#[utoipa::path(
    get,
    path = "/api/users/me/sessions",
    responses(
        (status = 200, description = "Active device sessions", body = [DeviceSession]),
        (status = 401, description = "Missing or expired session")
    )
)]
pub async fn list(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> Result<Json<Vec<DeviceSession>>, StatusCode> {
    let token = bearer_token(&headers)?;
    let user = state
        .session_user(token)
        .await
        .map_err(|error| {
            tracing::error!("load current session for device list failed: {error}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or(StatusCode::UNAUTHORIZED)?;
    state
        .device_sessions(user.id, token)
        .await
        .map(Json)
        .map_err(|error| {
            tracing::error!("list device sessions failed: {error}");
            StatusCode::INTERNAL_SERVER_ERROR
        })
}

/// Revoke one other device session owned by the current account.
#[utoipa::path(
    delete,
    path = "/api/users/me/sessions/{id}",
    params(("id" = String, Path, description = "Opaque device session identifier")),
    responses(
        (status = 204, description = "Device session revoked"),
        (status = 400, description = "Invalid device session identifier"),
        (status = 401, description = "Missing or expired session"),
        (status = 404, description = "Device session not found"),
        (status = 409, description = "Use logout to revoke the current session")
    )
)]
pub async fn revoke(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(management_id): Path<String>,
) -> Result<StatusCode, StatusCode> {
    if management_id.len() != 32 || !management_id.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(StatusCode::BAD_REQUEST);
    }
    let token = bearer_token(&headers)?;
    let user = state
        .session_user(token)
        .await
        .map_err(|error| {
            tracing::error!("load current session for device revocation failed: {error}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or(StatusCode::UNAUTHORIZED)?;
    match state
        .revoke_device_session(user.id, token, &management_id)
        .await
        .map_err(|error| {
            tracing::error!("revoke device session failed: {error}");
            StatusCode::INTERNAL_SERVER_ERROR
        })? {
        RevokeSessionResult::Revoked => Ok(StatusCode::NO_CONTENT),
        RevokeSessionResult::Current => Err(StatusCode::CONFLICT),
        RevokeSessionResult::NotFound => Err(StatusCode::NOT_FOUND),
    }
}

/// Revoke every device session except the current bearer session.
#[utoipa::path(
    delete,
    path = "/api/users/me/sessions/others",
    responses(
        (status = 204, description = "Other device sessions revoked"),
        (status = 401, description = "Missing or expired session")
    )
)]
pub async fn revoke_others(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> Result<StatusCode, StatusCode> {
    let token = bearer_token(&headers)?;
    let user = state
        .session_user(token)
        .await
        .map_err(|error| {
            tracing::error!("load current session for bulk revocation failed: {error}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or(StatusCode::UNAUTHORIZED)?;
    state
        .revoke_other_device_sessions(user.id, token)
        .await
        .map_err(|error| {
            tracing::error!("revoke other device sessions failed: {error}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(StatusCode::NO_CONTENT)
}

fn device_name(user_agent: &str) -> String {
    let ua = user_agent.to_ascii_lowercase();
    let browser = if ua.contains("edg/") {
        "Edge"
    } else if ua.contains("firefox/") || ua.contains("fxios/") {
        "Firefox"
    } else if ua.contains("chrome/") || ua.contains("crios/") {
        "Chrome"
    } else if ua.contains("safari/") {
        "Safari"
    } else {
        return "Unknown device".into();
    };
    let platform = if ua.contains("iphone") || ua.contains("ipad") {
        "iOS"
    } else if ua.contains("android") {
        "Android"
    } else if ua.contains("windows") {
        "Windows"
    } else if ua.contains("macintosh") || ua.contains("mac os") {
        "macOS"
    } else if ua.contains("linux") {
        "Linux"
    } else {
        "Unknown OS"
    };
    format!("{browser} on {platform}")
}

fn coarse_ip(address: &str) -> Option<String> {
    match address.parse::<IpAddr>().ok()? {
        IpAddr::V4(address) => {
            let [a, b, c, _] = address.octets();
            Some(format!("{a}.{b}.{c}.x"))
        }
        IpAddr::V6(address) => {
            let segments = address.segments();
            Some(format!(
                "{:x}:{:x}:{:x}:{:x}::/64",
                segments[0], segments[1], segments[2], segments[3]
            ))
        }
    }
}
