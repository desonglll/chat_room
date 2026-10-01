//! TG-501 chat folders: each user's folders and their rules (types, included/excluded chats,
//! exclusion flags), in order. Which chats a folder shows is evaluated by the client from its
//! conversation list (`packages/core/src/domain/chatFolders.ts`); the server owns storage,
//! limits and authorization — a folder may only name chats its owner can read, and chats the
//! owner later leaves drop out of what the API returns.

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use super::membership_handlers::session_user;
use crate::state::{with_pool, SharedState};

/// Telegram allows 10 folders; titles up to 12 characters.
pub const MAX_FOLDERS: i64 = 10;
pub const MAX_TITLE_CHARS: usize = 12;
const TYPES: [&str; 3] = ["private", "groups", "channels"];

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ChatFolder {
    pub id: Uuid,
    pub title: String,
    pub emoji: String,
    pub include_types: Vec<String>,
    pub include_chat_ids: Vec<Uuid>,
    pub exclude_chat_ids: Vec<Uuid>,
    pub exclude_muted: bool,
    pub exclude_read: bool,
    pub exclude_archived: bool,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ChatFolderWrite {
    pub title: String,
    #[serde(default)]
    pub emoji: String,
    #[serde(default)]
    pub include_types: Vec<String>,
    #[serde(default)]
    pub include_chat_ids: Vec<Uuid>,
    #[serde(default)]
    pub exclude_chat_ids: Vec<Uuid>,
    #[serde(default)]
    pub exclude_muted: bool,
    #[serde(default)]
    pub exclude_read: bool,
    #[serde(default)]
    pub exclude_archived: bool,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct FolderOrder {
    pub folder_ids: Vec<Uuid>,
}

mod store;

/// Validate a write for `user_id`: title, types, and that every named chat is readable.
async fn validate(
    state: &SharedState,
    user_id: Uuid,
    write: &ChatFolderWrite,
) -> Result<(), StatusCode> {
    let title = write.title.trim();
    if title.is_empty()
        || title.chars().count() > MAX_TITLE_CHARS
        || write.emoji.chars().count() > 4
    {
        return Err(StatusCode::BAD_REQUEST);
    }
    if write
        .include_types
        .iter()
        .any(|kind| !TYPES.contains(&kind.as_str()))
    {
        return Err(StatusCode::BAD_REQUEST);
    }
    if write.include_types.is_empty() && write.include_chat_ids.is_empty() {
        // An empty folder is pointless; Telegram requires at least one chat or type.
        return Err(StatusCode::BAD_REQUEST);
    }
    if write.include_chat_ids.len() + write.exclude_chat_ids.len() > 200 {
        return Err(StatusCode::BAD_REQUEST);
    }
    for room_id in write.include_chat_ids.iter().chain(&write.exclude_chat_ids) {
        if !state
            .can_read_chat(*room_id, user_id)
            .await
            .map_err(internal)?
        {
            return Err(StatusCode::NOT_FOUND);
        }
    }
    Ok(())
}

#[utoipa::path(get, path = "/api/users/me/folders",
    responses((status = 200, description = "The caller's folders, in order", body = Vec<ChatFolder>)))]
pub async fn list_folders(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> Result<Json<Vec<ChatFolder>>, StatusCode> {
    let user = session_user(&state, &headers).await?;
    Ok(Json(state.user_folders(user.id).await.map_err(internal)?))
}

#[utoipa::path(post, path = "/api/users/me/folders", request_body = ChatFolderWrite,
    responses((status = 201, description = "Created", body = ChatFolder),
        (status = 400, description = "Invalid title/types, empty folder, or the 10-folder limit"),
        (status = 404, description = "A named chat the caller cannot read")))]
pub async fn create_folder(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(write): Json<ChatFolderWrite>,
) -> Result<(StatusCode, Json<ChatFolder>), StatusCode> {
    let user = session_user(&state, &headers).await?;
    validate(&state, user.id, &write).await?;
    let count: i64 = with_pool!(state, |pool| {
        sqlx::query_scalar("SELECT COUNT(*) FROM chat_folders WHERE user_id = $1")
            .bind(user.id)
            .fetch_one(pool)
            .await
    })
    .map_err(internal)?;
    if count >= MAX_FOLDERS {
        return Err(StatusCode::BAD_REQUEST);
    }
    let id = Uuid::new_v4();
    let now = Utc::now();
    let types = serde_json::to_string(&write.include_types).unwrap_or_else(|_| "[]".into());
    with_pool!(state, |pool| {
        sqlx::query(
            "INSERT INTO chat_folders (id, user_id, title, emoji, position, include_types, exclude_muted, \
             exclude_read, exclude_archived, created_at, updated_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $10)",
        )
        .bind(id)
        .bind(user.id)
        .bind(write.title.trim())
        .bind(&write.emoji)
        .bind(count)
        .bind(&types)
        .bind(write.exclude_muted)
        .bind(write.exclude_read)
        .bind(write.exclude_archived)
        .bind(now)
        .execute(pool)
        .await
        .map(|_| ())
    })
    .map_err(internal)?;
    state
        .write_folder_chats(id, &write)
        .await
        .map_err(internal)?;
    let folder = state
        .user_folders(user.id)
        .await
        .map_err(internal)?
        .into_iter()
        .find(|folder| folder.id == id)
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok((StatusCode::CREATED, Json(folder)))
}

#[utoipa::path(put, path = "/api/users/me/folders/{id}", params(("id" = Uuid, Path, description = "Folder id")),
    request_body = ChatFolderWrite,
    responses((status = 200, description = "Replaced", body = ChatFolder), (status = 404, description = "Not yours")))]
pub async fn update_folder(
    State(state): State<SharedState>,
    Path(folder_id): Path<Uuid>,
    headers: HeaderMap,
    Json(write): Json<ChatFolderWrite>,
) -> Result<Json<ChatFolder>, StatusCode> {
    let user = session_user(&state, &headers).await?;
    validate(&state, user.id, &write).await?;
    let types = serde_json::to_string(&write.include_types).unwrap_or_else(|_| "[]".into());
    let updated = with_pool!(state, |pool| {
        sqlx::query(
            "UPDATE chat_folders SET title = $1, emoji = $2, include_types = $3, exclude_muted = $4, \
             exclude_read = $5, exclude_archived = $6, updated_at = $7 WHERE id = $8 AND user_id = $9",
        )
        .bind(write.title.trim())
        .bind(&write.emoji)
        .bind(&types)
        .bind(write.exclude_muted)
        .bind(write.exclude_read)
        .bind(write.exclude_archived)
        .bind(Utc::now())
        .bind(folder_id)
        .bind(user.id)
        .execute(pool)
        .await
        .map(|result| result.rows_affected())
    })
    .map_err(internal)?;
    if updated == 0 {
        return Err(StatusCode::NOT_FOUND);
    }
    state
        .write_folder_chats(folder_id, &write)
        .await
        .map_err(internal)?;
    state
        .user_folders(user.id)
        .await
        .map_err(internal)?
        .into_iter()
        .find(|folder| folder.id == folder_id)
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

#[utoipa::path(delete, path = "/api/users/me/folders/{id}", params(("id" = Uuid, Path, description = "Folder id")),
    responses((status = 204, description = "Deleted"), (status = 404, description = "Not yours")))]
pub async fn delete_folder(
    State(state): State<SharedState>,
    Path(folder_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<StatusCode, StatusCode> {
    let user = session_user(&state, &headers).await?;
    let deleted = with_pool!(state, |pool| {
        sqlx::query("DELETE FROM chat_folders WHERE id = $1 AND user_id = $2")
            .bind(folder_id)
            .bind(user.id)
            .execute(pool)
            .await
            .map(|result| result.rows_affected())
    })
    .map_err(internal)?;
    if deleted == 0 {
        return Err(StatusCode::NOT_FOUND);
    }
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(put, path = "/api/users/me/folders/order", request_body = FolderOrder,
    responses((status = 200, description = "Reordered", body = Vec<ChatFolder>),
        (status = 400, description = "Not exactly the caller's folders")))]
pub async fn reorder_folders(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(order): Json<FolderOrder>,
) -> Result<Json<Vec<ChatFolder>>, StatusCode> {
    let user = session_user(&state, &headers).await?;
    let mut current: Vec<Uuid> = state
        .user_folders(user.id)
        .await
        .map_err(internal)?
        .into_iter()
        .map(|folder| folder.id)
        .collect();
    let mut requested = order.folder_ids.clone();
    current.sort();
    requested.sort();
    if current != requested {
        return Err(StatusCode::BAD_REQUEST);
    }
    with_pool!(state, |pool| {
        async {
            let mut tx = pool.begin().await?;
            for (position, id) in order.folder_ids.iter().enumerate() {
                sqlx::query("UPDATE chat_folders SET position = $1 WHERE id = $2 AND user_id = $3")
                    .bind(position as i64)
                    .bind(id)
                    .bind(user.id)
                    .execute(&mut *tx)
                    .await?;
            }
            tx.commit().await
        }
        .await
    })
    .map_err(internal)?;
    Ok(Json(state.user_folders(user.id).await.map_err(internal)?))
}

fn internal(error: sqlx::Error) -> StatusCode {
    tracing::error!("chat folders: {error}");
    StatusCode::INTERNAL_SERVER_ERROR
}
