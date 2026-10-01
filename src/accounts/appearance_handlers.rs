//! HTTP surface of TG-507 wallpapers (`/api/users/me/wallpapers`). Translation only; the rules
//! are in `appearance.rs`. A chat scope must be a chat the caller can read.

use axum::{
    body::Body,
    extract::{Multipart, Path, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::Response,
    Json,
};
use tokio_util::io::ReaderStream;
use uuid::Uuid;

use super::appearance::{
    Wallpaper, WallpaperImage, WallpaperKind, WallpaperWrite, GLOBAL_SCOPE, MAX_DIM,
};
use super::avatar_handlers::detected_image_mime;
use crate::chats::membership_handlers::session_user;
use crate::models::User;
use crate::state::SharedState;

pub const MAX_WALLPAPER_BYTES: usize = 8 * 1024 * 1024;
pub const MULTIPART_OVERHEAD_BYTES: usize = 64 * 1024;

pub fn routes() -> axum::Router<std::sync::Arc<crate::state::AppState>> {
    use axum::routing::{get, put};
    axum::Router::new()
        .route("/api/users/me/wallpapers", get(list_wallpapers))
        .route(
            "/api/users/me/wallpapers/:scope",
            put(put_wallpaper).delete(delete_wallpaper),
        )
        .route(
            "/api/users/me/wallpapers/:scope/image",
            get(download_wallpaper_image)
                .post(upload_wallpaper_image)
                // Only this route accepts an 8 MiB body.
                .layer(axum::extract::DefaultBodyLimit::max(
                    MAX_WALLPAPER_BYTES + MULTIPART_OVERHEAD_BYTES,
                )),
        )
}

fn internal<E: std::fmt::Display>(error: E) -> StatusCode {
    tracing::error!("wallpaper request failed: {error}");
    StatusCode::INTERNAL_SERVER_ERROR
}

/// `global`, or a chat the caller can read (normalised to its id).
async fn checked_scope(
    state: &SharedState,
    user: &User,
    scope: &str,
) -> Result<String, StatusCode> {
    if scope == GLOBAL_SCOPE {
        return Ok(scope.to_string());
    }
    let chat_id: Uuid = scope.parse().map_err(|_| StatusCode::NOT_FOUND)?;
    if !state
        .can_read_chat(chat_id, user.id)
        .await
        .map_err(internal)?
    {
        return Err(StatusCode::NOT_FOUND);
    }
    Ok(chat_id.to_string())
}

#[utoipa::path(get, path = "/api/users/me/wallpapers",
    responses((status = 200, description = "Global and per-chat wallpapers", body = Vec<Wallpaper>)))]
pub async fn list_wallpapers(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> Result<Json<Vec<Wallpaper>>, StatusCode> {
    let user = session_user(&state, &headers).await?;
    state.wallpapers(user.id).await.map(Json).map_err(internal)
}

#[utoipa::path(put, path = "/api/users/me/wallpapers/{scope}", params(("scope" = String, description = "`global` or a chat id")),
    request_body = WallpaperWrite,
    responses((status = 200, description = "Stored", body = Wallpaper),
        (status = 400, description = "Unknown preset, bad colours, dim out of 0–80, or kind image"),
        (status = 404, description = "Not a chat the caller can read")))]
pub async fn put_wallpaper(
    State(state): State<SharedState>,
    Path(scope): Path<String>,
    headers: HeaderMap,
    Json(write): Json<WallpaperWrite>,
) -> Result<Json<Wallpaper>, StatusCode> {
    let user = session_user(&state, &headers).await?;
    let scope = checked_scope(&state, &user, &scope).await?;
    if !write.is_valid() {
        return Err(StatusCode::BAD_REQUEST);
    }
    let (stored, replaced) = state
        .put_wallpaper(user.id, &scope, &write, None)
        .await
        .map_err(internal)?;
    if let Some(old) = replaced {
        let _ = state.attachment_store().remove(&old).await;
    }
    Ok(Json(stored))
}

#[utoipa::path(post, path = "/api/users/me/wallpapers/{scope}/image", params(("scope" = String, description = "`global` or a chat id")),
    responses((status = 200, description = "Image wallpaper stored (multipart `file`, optional `blur`, `dim`)", body = Wallpaper),
        (status = 400, description = "Missing file or bad options"),
        (status = 413, description = "Image exceeds 8 MiB"),
        (status = 415, description = "Not a PNG, JPEG, GIF, WebP or AVIF image")))]
pub async fn upload_wallpaper_image(
    State(state): State<SharedState>,
    Path(scope): Path<String>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Result<Json<Wallpaper>, StatusCode> {
    let user = session_user(&state, &headers).await?;
    let scope = checked_scope(&state, &user, &scope).await?;
    let mut bytes: Option<Vec<u8>> = None;
    let mut write = WallpaperWrite {
        kind: WallpaperKind::Image,
        preset: String::new(),
        colors: Vec::new(),
        blur: false,
        dim: 0,
    };
    while let Some(mut field) = multipart
        .next_field()
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?
    {
        match field.name() {
            Some("file") if bytes.is_none() => {
                let mut data = Vec::new();
                while let Some(chunk) = field.chunk().await.map_err(|_| StatusCode::BAD_REQUEST)? {
                    if data.len().saturating_add(chunk.len()) > MAX_WALLPAPER_BYTES {
                        return Err(StatusCode::PAYLOAD_TOO_LARGE);
                    }
                    data.extend_from_slice(&chunk);
                }
                bytes = Some(data);
            }
            Some("blur") => {
                write.blur = field.text().await.map_err(|_| StatusCode::BAD_REQUEST)? == "true"
            }
            Some("dim") => {
                write.dim = field
                    .text()
                    .await
                    .ok()
                    .and_then(|text| text.parse().ok())
                    .ok_or(StatusCode::BAD_REQUEST)?;
            }
            _ => {}
        }
    }
    if !(0..=MAX_DIM).contains(&write.dim) {
        return Err(StatusCode::BAD_REQUEST);
    }
    let bytes = bytes
        .filter(|bytes| !bytes.is_empty())
        .ok_or(StatusCode::BAD_REQUEST)?;
    let mime_type = detected_image_mime(&bytes).ok_or(StatusCode::UNSUPPORTED_MEDIA_TYPE)?;
    let mut staged = state.attachment_store().begin().await.map_err(internal)?;
    staged.write(&bytes).await.map_err(internal)?;
    let storage_key = format!("wp{}", Uuid::new_v4().simple());
    let size_bytes = state
        .attachment_store()
        .commit(staged, &storage_key)
        .await
        .map_err(internal)?;
    let image = WallpaperImage {
        storage_key: storage_key.clone(),
        mime_type: mime_type.to_string(),
        size_bytes: size_bytes as i64,
    };
    let stored = match state
        .put_wallpaper(user.id, &scope, &write, Some(&image))
        .await
    {
        Ok((stored, replaced)) => {
            if let Some(old) = replaced {
                let _ = state.attachment_store().remove(&old).await;
            }
            stored
        }
        Err(error) => {
            let _ = state.attachment_store().remove(&storage_key).await;
            return Err(internal(error));
        }
    };
    Ok(Json(stored))
}

#[utoipa::path(delete, path = "/api/users/me/wallpapers/{scope}", params(("scope" = String, description = "`global` or a chat id")),
    responses((status = 204, description = "Back to the default (or to the global wallpaper, for a chat)"),
        (status = 404, description = "Nothing set for this scope")))]
pub async fn delete_wallpaper(
    State(state): State<SharedState>,
    Path(scope): Path<String>,
    headers: HeaderMap,
) -> Result<StatusCode, StatusCode> {
    let user = session_user(&state, &headers).await?;
    // A chat the caller has left can still have its override cleared.
    let scope = if scope == GLOBAL_SCOPE {
        scope
    } else {
        scope
            .parse::<Uuid>()
            .map_err(|_| StatusCode::NOT_FOUND)?
            .to_string()
    };
    let (removed, image) = state
        .delete_wallpaper(user.id, &scope)
        .await
        .map_err(internal)?;
    if let Some(key) = image {
        let _ = state.attachment_store().remove(&key).await;
    }
    if removed {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

#[utoipa::path(get, path = "/api/users/me/wallpapers/{scope}/image", params(("scope" = String, description = "`global` or a chat id")),
    responses((status = 200, description = "The owner's wallpaper image"), (status = 404, description = "No image wallpaper here")))]
pub async fn download_wallpaper_image(
    State(state): State<SharedState>,
    Path(scope): Path<String>,
    headers: HeaderMap,
) -> Result<Response<Body>, StatusCode> {
    let user = session_user(&state, &headers).await?;
    let image = state
        .wallpaper_image(user.id, &scope)
        .await
        .map_err(internal)?
        .ok_or(StatusCode::NOT_FOUND)?;
    let reader = state
        .attachment_store()
        .open_range(&image.storage_key, 0, image.size_bytes as u64)
        .await
        .map_err(|error| internal(format!("{error:#}")))?;
    let mut response = Response::new(Body::from_stream(ReaderStream::new(reader)));
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(&image.mime_type).map_err(internal)?,
    );
    headers.insert(
        header::CONTENT_LENGTH,
        HeaderValue::from(image.size_bytes as u64),
    );
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, max-age=86400"),
    );
    headers.insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    Ok(response)
}
