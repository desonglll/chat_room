use std::str::FromStr;

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use serde::Deserialize;

use super::models::{NotificationKind, NotificationPage, NotificationQuery, UnreadCount};
use crate::{state::SharedState, user_handlers::bearer_token};

const DEFAULT_LIMIT: i64 = 30;
const MAX_LIMIT: i64 = 50;

#[derive(Debug, Deserialize)]
pub struct NotificationParams {
    kind: Option<NotificationKind>,
    cursor: Option<String>,
    limit: Option<i64>,
}

async fn recipient_id(state: &SharedState, headers: &HeaderMap) -> Result<uuid::Uuid, StatusCode> {
    state
        .session_user(bearer_token(headers)?)
        .await
        .map_err(|error| {
            tracing::error!("load notification session failed: {error}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .map(|user| user.id)
        .ok_or(StatusCode::UNAUTHORIZED)
}

#[utoipa::path(
    get,
    path = "/api/notifications",
    params(
        ("kind" = Option<NotificationKind>, Query, description = "Optional notification kind"),
        ("cursor" = Option<String>, Query, description = "Exclusive stable result cursor"),
        ("limit" = Option<i64>, Query, description = "Results to return (1-50)")
    ),
    responses(
        (status = 200, description = "Notifications visible to the current account", body = NotificationPage),
        (status = 400, description = "Invalid cursor or page size"),
        (status = 401, description = "Missing or expired session")
    )
)]
pub async fn list(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Query(params): Query<NotificationParams>,
) -> Result<Json<NotificationPage>, StatusCode> {
    let recipient_id = recipient_id(&state, &headers).await?;
    let limit = params.limit.unwrap_or(DEFAULT_LIMIT);
    if !(1..=MAX_LIMIT).contains(&limit)
        || params
            .cursor
            .as_ref()
            .is_some_and(|cursor| cursor.chars().count() > 500)
    {
        return Err(StatusCode::BAD_REQUEST);
    }
    let cursor = params
        .cursor
        .as_deref()
        .map(super::models::NotificationCursor::from_str)
        .transpose()
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    state
        .list_notifications(
            recipient_id,
            &NotificationQuery {
                kind: params.kind,
                cursor,
                limit,
            },
        )
        .await
        .map(Json)
        .map_err(|error| {
            tracing::error!("list notifications failed: {error}");
            StatusCode::INTERNAL_SERVER_ERROR
        })
}

#[utoipa::path(
    get,
    path = "/api/notifications/unread-count",
    responses(
        (status = 200, description = "Current unread notification count", body = UnreadCount),
        (status = 401, description = "Missing or expired session")
    )
)]
pub async fn unread_count(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> Result<Json<UnreadCount>, StatusCode> {
    let recipient_id = recipient_id(&state, &headers).await?;
    state
        .notification_unread_count(recipient_id)
        .await
        .map(|unread_count| Json(UnreadCount { unread_count }))
        .map_err(|error| {
            tracing::error!("load notification unread count failed: {error}");
            StatusCode::INTERNAL_SERVER_ERROR
        })
}

#[utoipa::path(
    post,
    path = "/api/notifications/{id}/read",
    params(("id" = String, Path, description = "Notification identifier")),
    responses(
        (status = 204, description = "Notification marked read"),
        (status = 401, description = "Missing or expired session"),
        (status = 404, description = "Notification not found")
    )
)]
pub async fn mark_read(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<StatusCode, StatusCode> {
    if id.is_empty() || id.chars().count() > 300 {
        return Err(StatusCode::NOT_FOUND);
    }
    let recipient_id = recipient_id(&state, &headers).await?;
    state
        .mark_notification_read(recipient_id, &id)
        .await
        .map_err(|error| {
            tracing::error!("mark notification read failed: {error}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .then_some(StatusCode::NO_CONTENT)
        .ok_or(StatusCode::NOT_FOUND)
}

#[utoipa::path(
    post,
    path = "/api/notifications/read-all",
    responses(
        (status = 204, description = "All notifications marked read"),
        (status = 401, description = "Missing or expired session")
    )
)]
pub async fn mark_all_read(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> Result<StatusCode, StatusCode> {
    let recipient_id = recipient_id(&state, &headers).await?;
    state
        .mark_all_notifications_read(recipient_id)
        .await
        .map(|_| StatusCode::NO_CONTENT)
        .map_err(|error| {
            tracing::error!("mark all notifications read failed: {error}");
            StatusCode::INTERNAL_SERVER_ERROR
        })
}
