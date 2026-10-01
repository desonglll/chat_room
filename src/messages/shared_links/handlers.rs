//! HTTP translation for the link tab. No domain rules live here.

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use uuid::Uuid;

use super::SharedLinkPage;
use crate::file_handlers::authorize;
use crate::state::{with_pool, SharedState};

#[derive(Deserialize)]
pub struct LinkPageQuery {
    before: Option<String>,
    limit: Option<i64>,
}

#[utoipa::path(
    get,
    path = "/api/chats/{id}/links",
    params(
        ("id" = Uuid, description = "Chat id"),
        ("before" = Option<String>, Query, description = "The previous page's `next` cursor"),
        ("limit" = Option<i64>, Query, description = "Page size (1-100)")
    ),
    responses(
        (status = 200, description = "Links shared in the chat, newest first", body = SharedLinkPage),
        (status = 400, description = "Malformed cursor"),
        (status = 401, description = "Invalid credentials"),
        (status = 403, description = "Not an active chat member")
    )
)]
pub async fn list_shared_links(
    State(state): State<SharedState>,
    Path(room_id): Path<Uuid>,
    Query(query): Query<LinkPageQuery>,
    headers: HeaderMap,
) -> Result<Json<SharedLinkPage>, StatusCode> {
    authorize(&state, room_id, &headers).await?;
    let limit = query.limit.unwrap_or(50).clamp(1, 100);
    let before = match query.before.as_deref() {
        Some(cursor) => Some(resolve_cursor(&state, room_id, cursor).await?),
        None => None,
    };
    state
        .refresh_link_index(room_id)
        .await
        .map_err(database_error)?;
    let mut items = state
        .shared_links_page(room_id, before, limit + 1)
        .await
        .map_err(database_error)?;
    let has_more = items.len() > limit as usize;
    items.truncate(limit as usize);
    let next = has_more
        .then(|| {
            items
                .last()
                .map(|item| format!("{}:{}", item.message_id, item.position))
        })
        .flatten();
    Ok(Json(SharedLinkPage { items, next }))
}

/// `"<message id>:<position>"` → the row's sort key, checked to belong to this chat.
async fn resolve_cursor(
    state: &SharedState,
    room_id: Uuid,
    cursor: &str,
) -> Result<(DateTime<Utc>, Uuid, i32), StatusCode> {
    let (id, position) = cursor.split_once(':').ok_or(StatusCode::BAD_REQUEST)?;
    let id: Uuid = id.parse().map_err(|_| StatusCode::BAD_REQUEST)?;
    let position: i32 = position.parse().map_err(|_| StatusCode::BAD_REQUEST)?;
    let created_at: Option<DateTime<Utc>> = with_pool!(state, |pool| {
        sqlx::query_scalar(
            "SELECT created_at FROM message_links \
             WHERE message_id = $1 AND position = $2 AND room_id = $3",
        )
        .bind(id)
        .bind(position)
        .bind(room_id)
        .fetch_optional(pool)
        .await
    })
    .map_err(database_error)?;
    Ok((created_at.ok_or(StatusCode::BAD_REQUEST)?, id, position))
}

fn database_error(error: sqlx::Error) -> StatusCode {
    tracing::error!("list shared links failed: {error}");
    StatusCode::INTERNAL_SERVER_ERROR
}
