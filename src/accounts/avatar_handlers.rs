use axum::{
    body::Body,
    extract::{Multipart, Path, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::Response,
    Json,
};
use tokio_util::io::ReaderStream;
use uuid::Uuid;

use crate::models::User;
use crate::state::SharedState;
use crate::user_handlers::bearer_token;

pub const MAX_AVATAR_BYTES: usize = 5 * 1024 * 1024;
pub const MULTIPART_OVERHEAD_BYTES: usize = 64 * 1024;

#[utoipa::path(
    post,
    path = "/api/users/me/avatar",
    responses(
        (status = 200, description = "Avatar image saved", body = User),
        (status = 400, description = "Missing or unsupported image"),
        (status = 401, description = "Missing or expired session"),
        (status = 413, description = "Avatar exceeds 5 MiB")
    )
)]
pub async fn upload_avatar(
    State(state): State<SharedState>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Result<Json<User>, StatusCode> {
    let token = bearer_token(&headers)?;
    let current = state
        .session_user(token)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::UNAUTHORIZED)?;
    let mut image = None;
    while let Some(mut field) = multipart
        .next_field()
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?
    {
        if field.name() != Some("file") || image.is_some() {
            continue;
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = field.chunk().await.map_err(|_| StatusCode::BAD_REQUEST)? {
            if bytes.len().saturating_add(chunk.len()) > MAX_AVATAR_BYTES {
                return Err(StatusCode::PAYLOAD_TOO_LARGE);
            }
            bytes.extend_from_slice(&chunk);
        }
        image = Some(bytes);
    }
    let bytes = image
        .filter(|bytes| !bytes.is_empty())
        .ok_or(StatusCode::BAD_REQUEST)?;
    let mime_type = detected_image_mime(&bytes).ok_or(StatusCode::UNSUPPORTED_MEDIA_TYPE)?;
    let mut staged = state
        .attachment_store()
        .begin()
        .await
        .map_err(internal_error)?;
    staged.write(&bytes).await.map_err(internal_error)?;
    let storage_key = format!("av{}", Uuid::new_v4().simple());
    let size_bytes = state
        .attachment_store()
        .commit(staged, &storage_key)
        .await
        .map_err(internal_error)?;
    let version = Uuid::new_v4().simple().to_string();
    let avatar_url = format!("/api/users/{}/avatar?v={version}", current.id);
    let stored = state
        .replace_user_avatar_file(current.id, &storage_key, mime_type, size_bytes, &avatar_url)
        .await;
    let (old_key, updated) = match stored {
        Ok(Some(value)) => value,
        Ok(None) => {
            let _ = state.attachment_store().remove(&storage_key).await;
            return Err(StatusCode::UNAUTHORIZED);
        }
        Err(error) => {
            tracing::error!("save avatar metadata failed: {error}");
            let _ = state.attachment_store().remove(&storage_key).await;
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };
    if let Some(old_key) = old_key.filter(|old_key| old_key != &storage_key) {
        if let Err(error) = state.attachment_store().remove(&old_key).await {
            tracing::warn!("remove replaced avatar failed: {error:#}");
        }
    }
    state.publish_member_profile(&updated).await;
    Ok(Json(updated))
}

#[utoipa::path(
    get,
    path = "/api/users/{id}/avatar",
    params(("id" = Uuid, Path, description = "Account id")),
    responses(
        (status = 200, description = "Avatar image"),
        (status = 404, description = "Avatar not found")
    )
)]
pub async fn download_avatar(
    State(state): State<SharedState>,
    Path(user_id): Path<Uuid>,
) -> Response<Body> {
    let avatar = match state.avatar_file(user_id).await {
        Ok(Some(avatar)) => avatar,
        Ok(None) => return empty_response(StatusCode::NOT_FOUND),
        Err(error) => {
            tracing::error!("load avatar metadata failed: {error}");
            return empty_response(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };
    let reader = match state
        .attachment_store()
        .open_range(&avatar.storage_key, 0, avatar.size_bytes as u64)
        .await
    {
        Ok(reader) => reader,
        Err(error) => {
            tracing::error!("open avatar data failed: {error:#}");
            return empty_response(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };
    let mut response = Response::new(Body::from_stream(ReaderStream::new(reader)));
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(&avatar.mime_type)
            .unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream")),
    );
    response.headers_mut().insert(
        header::CONTENT_LENGTH,
        HeaderValue::from_str(&avatar.size_bytes.to_string()).unwrap(),
    );
    response.headers_mut().insert(
        header::LAST_MODIFIED,
        HeaderValue::from_str(&avatar.updated_at.to_rfc2822()).unwrap(),
    );
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=31536000, immutable"),
    );
    response.headers_mut().insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    response
}

fn detected_image_mime(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if bytes.starts_with(b"\xff\xd8\xff") {
        Some("image/jpeg")
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some("image/gif")
    } else if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("image/webp")
    } else if bytes.len() >= 12
        && &bytes[4..8] == b"ftyp"
        && matches!(&bytes[8..12], b"avif" | b"avis")
    {
        Some("image/avif")
    } else {
        None
    }
}

fn internal_error(error: anyhow::Error) -> StatusCode {
    tracing::error!("avatar storage failed: {error:#}");
    StatusCode::INTERNAL_SERVER_ERROR
}

fn empty_response(status: StatusCode) -> Response<Body> {
    Response::builder()
        .status(status)
        .body(Body::empty())
        .unwrap()
}
