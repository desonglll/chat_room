//! TG-511 profile photo history: page through a user's photos, set an older one as the main
//! photo, delete one. Every read goes through the TG-505 profile-photo rule, like the current
//! avatar. Deleting the main photo promotes the next newest one (or leaves no photo), so the
//! order stays newest first and there is never a pointer to a deleted file.

use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, HeaderMap, HeaderValue, Response, StatusCode},
    Json,
};
use chrono::{DateTime, Utc};
use serde::Serialize;
use tokio_util::io::ReaderStream;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::models::User;
use crate::state::{with_pool, AppState, SharedState};
use crate::user_handlers::{bearer_token, optional_bearer_token};

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct AvatarHistoryEntry {
    /// Stable id of this photo.
    pub id: String,
    pub url: String,
    pub is_current: bool,
    pub created_at: DateTime<Utc>,
}

struct HistoryRow {
    storage_key: String,
    mime_type: String,
    size_bytes: i64,
    created_at: DateTime<Utc>,
}

impl AppState {
    async fn avatar_history_rows(&self, user_id: Uuid) -> Result<Vec<HistoryRow>, sqlx::Error> {
        let rows: Vec<(String, String, i64, DateTime<Utc>)> = with_pool!(self, |pool| {
            sqlx::query_as(
                "SELECT storage_key, mime_type, size_bytes, created_at FROM user_avatar_history \
                 WHERE user_id = $1 ORDER BY created_at DESC, storage_key DESC",
            )
            .bind(user_id)
            .fetch_all(pool)
            .await
        })?;
        Ok(rows
            .into_iter()
            .map(
                |(storage_key, mime_type, size_bytes, created_at)| HistoryRow {
                    storage_key,
                    mime_type,
                    size_bytes,
                    created_at,
                },
            )
            .collect())
    }

    async fn current_avatar_key(&self, user_id: Uuid) -> Result<Option<String>, sqlx::Error> {
        Ok(self
            .avatar_file(user_id)
            .await?
            .map(|file| file.storage_key))
    }

    /// Make `row` the main photo: the pointer and the users' `avatar_emoji` URL.
    async fn set_main_avatar(
        &self,
        user_id: Uuid,
        row: &HistoryRow,
    ) -> Result<Option<User>, sqlx::Error> {
        let url = format!("/api/users/{user_id}/avatar?v={}", Uuid::new_v4().simple());
        let now = Utc::now();
        let user = with_pool!(self, |pool| {
            async {
                let mut tx = pool.begin().await?;
                sqlx::query(
                    "INSERT INTO user_avatar_files (user_id, storage_key, mime_type, size_bytes, updated_at) \
                     VALUES ($1, $2, $3, $4, $5) ON CONFLICT(user_id) DO UPDATE SET \
                     storage_key = excluded.storage_key, mime_type = excluded.mime_type, \
                     size_bytes = excluded.size_bytes, updated_at = excluded.updated_at",
                )
                .bind(user_id)
                .bind(&row.storage_key)
                .bind(&row.mime_type)
                .bind(row.size_bytes)
                .bind(now)
                .execute(&mut *tx)
                .await?;
                let user: Option<User> = sqlx::query_as(
                    "UPDATE users SET avatar_emoji = $1 WHERE id = $2 \
                     RETURNING id, username, avatar_emoji, display_name, signature, homepage, created_at",
                )
                .bind(&url)
                .bind(user_id)
                .fetch_optional(&mut *tx)
                .await?;
                tx.commit().await?;
                Ok::<_, sqlx::Error>(user)
            }
            .await
        })?;
        self.invalidate_user_sessions(user_id).await;
        Ok(user)
    }

    /// No photo left: clear the pointer and fall back to the default emoji.
    async fn clear_main_avatar(&self, user_id: Uuid) -> Result<Option<User>, sqlx::Error> {
        let user = with_pool!(self, |pool| {
            async {
                let mut tx = pool.begin().await?;
                sqlx::query("DELETE FROM user_avatar_files WHERE user_id = $1")
                    .bind(user_id)
                    .execute(&mut *tx)
                    .await?;
                let user: Option<User> = sqlx::query_as(
                    "UPDATE users SET avatar_emoji = '' WHERE id = $1 \
                     RETURNING id, username, avatar_emoji, display_name, signature, homepage, created_at",
                )
                .bind(user_id)
                .fetch_optional(&mut *tx)
                .await?;
                tx.commit().await?;
                Ok::<_, sqlx::Error>(user)
            }
            .await
        })?;
        self.invalidate_user_sessions(user_id).await;
        Ok(user)
    }
}

async fn viewer(state: &SharedState, headers: &HeaderMap) -> Result<Option<Uuid>, StatusCode> {
    match optional_bearer_token(headers) {
        Some(token) => Ok(state
            .session_user(token)
            .await
            .map_err(internal)?
            .map(|user| user.id)),
        None => Ok(None),
    }
}

async fn owner(state: &SharedState, headers: &HeaderMap) -> Result<User, StatusCode> {
    let token = bearer_token(headers)?;
    state
        .session_user(token)
        .await
        .map_err(internal)?
        .ok_or(StatusCode::UNAUTHORIZED)
}

#[utoipa::path(get, path = "/api/users/{id}/avatars", params(("id" = Uuid, Path, description = "Account id")),
    responses((status = 200, description = "Profile photos, newest first", body = Vec<AvatarHistoryEntry>),
        (status = 404, description = "Hidden by the owner's privacy rule")))]
pub async fn list_avatars(
    State(state): State<SharedState>,
    Path(user_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Vec<AvatarHistoryEntry>>, StatusCode> {
    let viewer = viewer(&state, &headers).await?;
    if !state
        .profile_photo_visible(user_id, viewer)
        .await
        .map_err(internal)?
    {
        return Err(StatusCode::NOT_FOUND);
    }
    let current = state.current_avatar_key(user_id).await.map_err(internal)?;
    let rows = state.avatar_history_rows(user_id).await.map_err(internal)?;
    Ok(Json(
        rows.into_iter()
            .map(|row| AvatarHistoryEntry {
                url: format!("/api/users/{user_id}/avatars/{}", row.storage_key),
                is_current: current.as_deref() == Some(row.storage_key.as_str()),
                id: row.storage_key,
                created_at: row.created_at,
            })
            .collect(),
    ))
}

#[utoipa::path(get, path = "/api/users/{id}/avatars/{avatar_id}",
    params(("id" = Uuid, Path, description = "Account id"), ("avatar_id" = String, Path, description = "Photo id")),
    responses((status = 200, description = "The photo"), (status = 404, description = "Not found or hidden")))]
pub async fn download_history_avatar(
    State(state): State<SharedState>,
    Path((user_id, avatar_id)): Path<(Uuid, String)>,
    headers: HeaderMap,
) -> Result<Response<Body>, StatusCode> {
    let viewer = viewer(&state, &headers).await?;
    if !state
        .profile_photo_visible(user_id, viewer)
        .await
        .map_err(internal)?
    {
        return Err(StatusCode::NOT_FOUND);
    }
    let row = state
        .avatar_history_rows(user_id)
        .await
        .map_err(internal)?
        .into_iter()
        .find(|row| row.storage_key == avatar_id)
        .ok_or(StatusCode::NOT_FOUND)?;
    let reader = state
        .attachment_store()
        .open_range(&row.storage_key, 0, row.size_bytes as u64)
        .await
        .map_err(|error| {
            tracing::error!("open avatar history file failed: {error:#}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    let mut response = Response::new(Body::from_stream(ReaderStream::new(reader)));
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(&row.mime_type)
            .unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream")),
    );
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static(if viewer.is_some() {
            "private, max-age=300"
        } else {
            "public, max-age=300"
        }),
    );
    headers.insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    Ok(response)
}

#[utoipa::path(put, path = "/api/users/me/avatars/{avatar_id}/main",
    params(("avatar_id" = String, Path, description = "Photo id")),
    responses((status = 200, description = "Now the main photo", body = User), (status = 404, description = "Not yours")))]
pub async fn set_main(
    State(state): State<SharedState>,
    Path(avatar_id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<User>, StatusCode> {
    let me = owner(&state, &headers).await?;
    let row = state
        .avatar_history_rows(me.id)
        .await
        .map_err(internal)?
        .into_iter()
        .find(|row| row.storage_key == avatar_id)
        .ok_or(StatusCode::NOT_FOUND)?;
    let updated = state
        .set_main_avatar(me.id, &row)
        .await
        .map_err(internal)?
        .ok_or(StatusCode::UNAUTHORIZED)?;
    state.publish_member_profile(&updated).await;
    Ok(Json(updated))
}

#[utoipa::path(delete, path = "/api/users/me/avatars/{avatar_id}",
    params(("avatar_id" = String, Path, description = "Photo id")),
    responses((status = 200, description = "Deleted; the next newest photo is main now (if any)", body = User),
        (status = 404, description = "Not yours")))]
pub async fn delete_avatar(
    State(state): State<SharedState>,
    Path(avatar_id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<User>, StatusCode> {
    let me = owner(&state, &headers).await?;
    let rows = state.avatar_history_rows(me.id).await.map_err(internal)?;
    if !rows.iter().any(|row| row.storage_key == avatar_id) {
        return Err(StatusCode::NOT_FOUND);
    }
    let was_current = state
        .current_avatar_key(me.id)
        .await
        .map_err(internal)?
        .as_deref()
        == Some(avatar_id.as_str());
    with_pool!(state, |pool| {
        sqlx::query("DELETE FROM user_avatar_history WHERE storage_key = $1 AND user_id = $2")
            .bind(&avatar_id)
            .bind(me.id)
            .execute(pool)
            .await
            .map(|_| ())
    })
    .map_err(internal)?;
    let updated = if was_current {
        match rows.iter().find(|row| row.storage_key != avatar_id) {
            Some(next) => state.set_main_avatar(me.id, next).await,
            None => state.clear_main_avatar(me.id).await,
        }
        .map_err(internal)?
        .ok_or(StatusCode::UNAUTHORIZED)?
    } else {
        me
    };
    if let Err(error) = state.attachment_store().remove(&avatar_id).await {
        tracing::warn!("remove deleted avatar file failed: {error:#}");
    }
    state.publish_member_profile(&updated).await;
    Ok(Json(updated))
}

fn internal(error: sqlx::Error) -> StatusCode {
    tracing::error!("avatar history: {error}");
    StatusCode::INTERNAL_SERVER_ERROR
}
