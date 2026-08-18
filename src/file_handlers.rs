//! Authenticated, independently paginated room attachment listing.

use axum::{
    extract::{Path, Query, State},
    http::{header::AUTHORIZATION, HeaderMap, StatusCode},
    Json,
};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use sqlx::FromRow;
use uuid::Uuid;

use crate::models::{Attachment, ChatFileItem, ChatFilePage};
use crate::state::{with_pool, SharedState};

#[derive(Deserialize)]
pub struct FilePageQuery {
    before: Option<Uuid>,
    limit: Option<i64>,
    kind: Option<String>,
}

#[derive(FromRow)]
struct FileRow {
    message_id: Uuid,
    sender_id: Option<Uuid>,
    sender: String,
    sender_avatar: String,
    created_at: DateTime<Utc>,
    attachment_id: Uuid,
    access_key: Uuid,
    file_name: String,
    mime_type: String,
    size_bytes: i64,
}

impl FileRow {
    fn into_item(self) -> ChatFileItem {
        ChatFileItem {
            message_id: self.message_id,
            sender_id: self.sender_id,
            sender: self.sender,
            sender_avatar: self.sender_avatar,
            created_at: self.created_at,
            attachment: Attachment {
                id: self.attachment_id,
                file_name: self.file_name,
                mime_type: self.mime_type,
                size_bytes: self.size_bytes,
                download_url: format!(
                    "/api/attachments/{}?key={}",
                    self.attachment_id, self.access_key
                ),
            },
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/rooms/{id}/files",
    params(
        ("id" = Uuid, description = "Room id"),
        ("before" = Option<Uuid>, Query, description = "Exclusive message cursor"),
        ("limit" = Option<i64>, Query, description = "Page size (1-100)"),
        ("kind" = Option<String>, Query, description = "all, image, video, or file")
    ),
    responses(
        (status = 200, description = "Paginated room files", body = ChatFilePage),
        (status = 401, description = "Invalid credentials"),
        (status = 403, description = "Not an active room member")
    )
)]
pub async fn list_room_files(
    State(state): State<SharedState>,
    Path(room_id): Path<Uuid>,
    Query(query): Query<FilePageQuery>,
    headers: HeaderMap,
) -> Result<Json<ChatFilePage>, StatusCode> {
    authorize(&state, room_id, &headers).await?;
    let kind = query.kind.as_deref().unwrap_or("all");
    if !matches!(kind, "all" | "image" | "video" | "file") {
        return Err(StatusCode::BAD_REQUEST);
    }
    let limit = query.limit.unwrap_or(50).clamp(1, 100);
    let before = match query.before {
        Some(message_id) => {
            let created_at = with_pool!(state, |pool| { sqlx::query_scalar::<_, DateTime<Utc>>(
                "SELECT created_at FROM messages WHERE id = $1 AND room_id = $2",
            )
            .bind(message_id)
            .bind(room_id)
            .fetch_optional(pool)
            .await })
            .map_err(database_error)?
            .ok_or(StatusCode::BAD_REQUEST)?;
            Some((created_at, message_id))
        }
        None => None,
    };

    let mut rows = fetch_page(&state, room_id, kind, before, limit + 1).await?;
    let has_more = rows.len() > limit as usize;
    rows.truncate(limit as usize);
    let next_before = has_more
        .then(|| rows.last().map(|row| row.message_id))
        .flatten();
    Ok(Json(ChatFilePage {
        items: rows.into_iter().map(FileRow::into_item).collect(),
        next_before,
    }))
}

async fn fetch_page(
    state: &SharedState,
    room_id: Uuid,
    kind: &str,
    before: Option<(DateTime<Utc>, Uuid)>,
    limit: i64,
) -> Result<Vec<FileRow>, StatusCode> {
    let (cursor_clause, kind_parameters, limit_parameter) = if before.is_some() {
        ("AND (messages.created_at < $2 OR (messages.created_at = $3 AND messages.id < $4))", ["$5", "$6", "$7", "$8"], "$9")
    } else {
        ("", ["$2", "$3", "$4", "$5"], "$6")
    };
    let sql = format!(
        "SELECT messages.id AS message_id, messages.sender_id, messages.sender, \
         COALESCE(users.avatar_emoji, '') AS sender_avatar, messages.created_at, \
         attachments.id AS attachment_id, attachments.access_key, attachments.file_name, \
         attachments.mime_type, attachments.size_bytes FROM messages \
         JOIN attachments ON attachments.id = messages.attachment_id \
         LEFT JOIN users ON users.id = messages.sender_id \
         WHERE messages.room_id = $1 AND messages.recalled_at IS NULL {cursor_clause} \
         AND ({0} = 'all' OR ({1} = 'image' AND attachments.mime_type LIKE 'image/%') \
           OR ({2} = 'video' AND attachments.mime_type LIKE 'video/%') \
           OR ({3} = 'file' AND attachments.mime_type NOT LIKE 'image/%' \
                         AND attachments.mime_type NOT LIKE 'video/%')) \
         ORDER BY messages.created_at DESC, messages.id DESC LIMIT {limit_parameter}",
        kind_parameters[0], kind_parameters[1], kind_parameters[2], kind_parameters[3]
    );
    with_pool!(state, |pool| {
        let query = sqlx::query_as::<_, FileRow>(&sql).bind(room_id);
        let query = match before {
            Some((created_at, id)) => query.bind(created_at).bind(created_at).bind(id),
            None => query,
        };
        query
            .bind(kind)
            .bind(kind)
            .bind(kind)
            .bind(kind)
            .bind(limit)
            .fetch_all(pool)
            .await
    })
        .map_err(database_error)
}

async fn authorize(
    state: &SharedState,
    room_id: Uuid,
    headers: &HeaderMap,
) -> Result<(), StatusCode> {
    let room = state.room(room_id).await.ok_or(StatusCode::NOT_FOUND)?;
    let token = headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .and_then(|value| value.parse().ok())
        .ok_or(StatusCode::UNAUTHORIZED)?;
    let user = state
        .session_user(token)
        .await
        .map_err(database_error)?
        .ok_or(StatusCode::UNAUTHORIZED)?;
    let allowed = state
        .has_room_permission(room_id, user.id, "message.send")
        .await
        .map_err(database_error)?;
    if !allowed {
        return Err(StatusCode::FORBIDDEN);
    }
    if room.has_password {
        let supplied = headers
            .get("x-room-password")
            .and_then(|value| value.to_str().ok())
            .ok_or(StatusCode::UNAUTHORIZED)?;
        let digest = Sha256::digest(supplied.as_bytes());
        if hex::encode(digest) != room.password_hash {
            return Err(StatusCode::UNAUTHORIZED);
        }
    }
    Ok(())
}

fn database_error(error: sqlx::Error) -> StatusCode {
    tracing::error!("list room files failed: {error}");
    StatusCode::INTERNAL_SERVER_ERROR
}
