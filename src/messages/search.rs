//! Authorized room-message search and targeted context loading.

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use uuid::Uuid;

use super::store::{MessageCursor, MessageRow, MESSAGE_SELECT};
use crate::{
    models::{StoredMessage, User},
    rooms::handlers::authorize_room,
    state::{with_pool, AppState, SharedState},
    user_handlers::bearer_token,
};

const DEFAULT_SEARCH_LIMIT: i64 = 50;
const DEFAULT_CONTEXT_LIMIT: i64 = 60;

#[derive(Debug, Deserialize)]
pub struct MessageSearchQuery {
    q: String,
    before: Option<Uuid>,
    limit: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct MessageContextQuery {
    limit: Option<i64>,
}

#[utoipa::path(
    get,
    path = "/api/rooms/{id}/messages/search",
    params(
        ("id" = Uuid, Path, description = "Room id"),
        ("q" = String, Query, description = "Text to find in message content"),
        ("before" = Option<Uuid>, Query, description = "Exclusive result cursor"),
        ("limit" = Option<i64>, Query, description = "Results to return (1-100)"),
        ("x-room-password" = Option<String>, Header, description = "Required for private rooms")
    ),
    responses(
        (status = 200, description = "Matching room messages, newest first", body = Vec<StoredMessage>),
        (status = 400, description = "Invalid query or cursor"),
        (status = 401, description = "Missing session or room password"),
        (status = 403, description = "Not an active room member"),
        (status = 404, description = "Room not found")
    )
)]
pub async fn search_messages(
    State(state): State<SharedState>,
    Path(room_id): Path<Uuid>,
    Query(query): Query<MessageSearchQuery>,
    headers: HeaderMap,
) -> Result<Json<Vec<StoredMessage>>, StatusCode> {
    let user = authorized_viewer(&state, room_id, &headers).await?;
    let text = query.q.trim();
    if text.is_empty() || text.chars().count() > 200 {
        return Err(StatusCode::BAD_REQUEST);
    }
    let before = resolve_cursor(&state, room_id, query.before).await?;
    state
        .search_room_messages(
            room_id,
            text,
            query.limit.unwrap_or(DEFAULT_SEARCH_LIMIT),
            before.as_ref(),
            user.id,
        )
        .await
        .map(Json)
        .map_err(internal_error)
}

#[utoipa::path(
    get,
    path = "/api/rooms/{id}/messages/{message_id}/context",
    params(
        ("id" = Uuid, Path, description = "Room id"),
        ("message_id" = Uuid, Path, description = "Target message id"),
        ("limit" = Option<i64>, Query, description = "Context messages to return (1-100)"),
        ("x-room-password" = Option<String>, Header, description = "Required for private rooms")
    ),
    responses(
        (status = 200, description = "Messages surrounding the target", body = Vec<StoredMessage>),
        (status = 401, description = "Missing session or room password"),
        (status = 403, description = "Not an active room member"),
        (status = 404, description = "Room or target message not found")
    )
)]
pub async fn message_context(
    State(state): State<SharedState>,
    Path((room_id, message_id)): Path<(Uuid, Uuid)>,
    Query(query): Query<MessageContextQuery>,
    headers: HeaderMap,
) -> Result<Json<Vec<StoredMessage>>, StatusCode> {
    let user = authorized_viewer(&state, room_id, &headers).await?;
    state
        .message_context(
            room_id,
            message_id,
            query.limit.unwrap_or(DEFAULT_CONTEXT_LIMIT),
            user.id,
        )
        .await
        .map_err(internal_error)?
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

impl AppState {
    async fn search_room_messages(
        &self,
        room_id: Uuid,
        text: &str,
        limit: i64,
        before: Option<&MessageCursor>,
        viewer_id: Uuid,
    ) -> Result<Vec<StoredMessage>, sqlx::Error> {
        let limit = limit.clamp(1, 100);
        let pattern = like_pattern(text);
        let query = match before {
            Some(_) => format!(
                "{MESSAGE_SELECT} WHERE messages.room_id = $1 AND messages.recalled_at IS NULL \
                 AND LOWER(messages.content) LIKE LOWER($2) ESCAPE '\\' AND \
                 (messages.created_at < $3 OR (messages.created_at = $4 AND messages.id < $5)) \
                 ORDER BY messages.created_at DESC, messages.id DESC LIMIT $6"
            ),
            None => format!(
                "{MESSAGE_SELECT} WHERE messages.room_id = $1 AND messages.recalled_at IS NULL \
                 AND LOWER(messages.content) LIKE LOWER($2) ESCAPE '\\' \
                 ORDER BY messages.created_at DESC, messages.id DESC LIMIT $3"
            ),
        };
        let rows: Vec<MessageRow> = with_pool!(self, |pool| {
            match before {
                Some(cursor) => {
                    sqlx::query_as(&query)
                        .bind(room_id)
                        .bind(&pattern)
                        .bind(cursor.created_at)
                        .bind(cursor.created_at)
                        .bind(cursor.id)
                        .bind(limit)
                        .fetch_all(pool)
                        .await
                }
                None => {
                    sqlx::query_as(&query)
                        .bind(room_id)
                        .bind(&pattern)
                        .bind(limit)
                        .fetch_all(pool)
                        .await
                }
            }
        })?;
        let mut messages = rows
            .into_iter()
            .map(|row| row.into_message(Some(viewer_id)))
            .collect::<Vec<_>>();
        self.attach_message_reactions(&mut messages).await?;
        Ok(messages)
    }

    async fn message_context(
        &self,
        room_id: Uuid,
        message_id: Uuid,
        limit: i64,
        viewer_id: Uuid,
    ) -> Result<Option<Vec<StoredMessage>>, sqlx::Error> {
        let Some(cursor) = message_cursor(self, room_id, message_id).await? else {
            return Ok(None);
        };
        let limit = limit.clamp(1, 100);
        let older_limit = limit / 2 + 1;
        let older_query = format!(
            "{MESSAGE_SELECT} WHERE messages.room_id = $1 AND \
             (messages.created_at < $2 OR (messages.created_at = $3 AND messages.id <= $4)) \
             ORDER BY messages.created_at DESC, messages.id DESC LIMIT $5"
        );
        let mut older: Vec<MessageRow> = with_pool!(self, |pool| {
            sqlx::query_as(&older_query)
                .bind(room_id)
                .bind(cursor.created_at)
                .bind(cursor.created_at)
                .bind(cursor.id)
                .bind(older_limit)
                .fetch_all(pool)
                .await
        })?;
        older.reverse();
        let newer_limit = limit.saturating_sub(older.len() as i64);
        let newer_query = format!(
            "{MESSAGE_SELECT} WHERE messages.room_id = $1 AND \
             (messages.created_at > $2 OR (messages.created_at = $3 AND messages.id > $4)) \
             ORDER BY messages.created_at ASC, messages.id ASC LIMIT $5"
        );
        let newer: Vec<MessageRow> = with_pool!(self, |pool| {
            sqlx::query_as(&newer_query)
                .bind(room_id)
                .bind(cursor.created_at)
                .bind(cursor.created_at)
                .bind(cursor.id)
                .bind(newer_limit)
                .fetch_all(pool)
                .await
        })?;
        let mut messages = older
            .into_iter()
            .chain(newer)
            .map(|row| row.into_message(Some(viewer_id)))
            .collect::<Vec<_>>();
        self.attach_message_reactions(&mut messages).await?;
        Ok(Some(messages))
    }
}

async fn authorized_viewer(
    state: &AppState,
    room_id: Uuid,
    headers: &HeaderMap,
) -> Result<User, StatusCode> {
    let room = state.room(room_id).await.ok_or(StatusCode::NOT_FOUND)?;
    let user = state
        .session_user(bearer_token(headers)?)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::UNAUTHORIZED)?;
    if !state
        .has_room_permission(room_id, user.id, "message.send")
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    {
        return Err(StatusCode::FORBIDDEN);
    }
    let supplied = headers
        .get("x-room-password")
        .and_then(|value| value.to_str().ok());
    authorize_room(&room, supplied)
        .then_some(user)
        .ok_or(StatusCode::UNAUTHORIZED)
}

async fn resolve_cursor(
    state: &AppState,
    room_id: Uuid,
    message_id: Option<Uuid>,
) -> Result<Option<MessageCursor>, StatusCode> {
    let Some(message_id) = message_id else {
        return Ok(None);
    };
    message_cursor(state, room_id, message_id)
        .await
        .map_err(internal_error)?
        .map(Some)
        .ok_or(StatusCode::BAD_REQUEST)
}

async fn message_cursor(
    state: &AppState,
    room_id: Uuid,
    message_id: Uuid,
) -> Result<Option<MessageCursor>, sqlx::Error> {
    let created_at: Option<DateTime<Utc>> = with_pool!(state, |pool| {
        sqlx::query_scalar("SELECT created_at FROM messages WHERE id = $1 AND room_id = $2")
            .bind(message_id)
            .bind(room_id)
            .fetch_optional(pool)
            .await
    })?;
    Ok(created_at.map(|created_at| MessageCursor {
        created_at,
        id: message_id,
    }))
}

fn like_pattern(text: &str) -> String {
    let escaped = text
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    format!("%{escaped}%")
}

fn internal_error(error: sqlx::Error) -> StatusCode {
    tracing::error!("room message search failed: {error}");
    StatusCode::INTERNAL_SERVER_ERROR
}

#[cfg(test)]
mod tests {
    use super::like_pattern;

    #[test]
    fn search_pattern_escapes_sql_wildcards() {
        assert_eq!(like_pattern(r"50%_done\ok"), r"%50\%\_done\\ok%");
    }
}
