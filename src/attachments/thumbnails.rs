//! TG-1302: small server-generated previews of image attachments.
//!
//! A thumbnail is a projection of the original, which stays the only source of truth: it is
//! rendered on first request, cached under the attachment store's `.thumbnails` directory
//! (local even with OSS — losing it only costs a re-render), and served under exactly the
//! original's authorisation, `attachment_metadata(id, key)`: the same capability key, and gone
//! the moment the original is (recalled message, deleted favourite).

use std::path::PathBuf;
use std::sync::LazyLock;

use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{
        header::{CACHE_CONTROL, CONTENT_LENGTH, CONTENT_TYPE},
        HeaderValue, Response, StatusCode,
    },
};
use tokio::io::AsyncReadExt;
use tokio::sync::Semaphore;
use uuid::Uuid;

use super::handlers::AttachmentAccess;
use super::thumbnail_render::{render_thumbnail, stored_content_type, RenderError};
use crate::state::SharedState;

/// Originals larger than this are not thumbnailed (404, so the client shows the original).
pub const MAX_SOURCE_BYTES: i64 = 32 * 1024 * 1024;
/// Decoding is CPU- and memory-heavy; at most this many renders run at once server-wide.
static RENDERS: LazyLock<Semaphore> = LazyLock::new(|| Semaphore::new(2));

/// Raster types we can decode. SVG is deliberately absent (it is never rendered server-side).
const THUMBNAILABLE: [&str; 5] = [
    "image/jpeg",
    "image/jpg",
    "image/png",
    "image/gif",
    "image/webp",
];

fn thumbnailable(mime_type: &str) -> bool {
    let mime_type = mime_type.split(';').next().unwrap_or_default().trim();
    THUMBNAILABLE
        .iter()
        .any(|candidate| candidate.eq_ignore_ascii_case(mime_type))
}

/// The thumbnail URL for an attachment, or `None` when its type has none.
pub fn thumbnail_url(id: Uuid, mime_type: &str, access_key: Uuid) -> Option<String> {
    thumbnailable(mime_type).then(|| format!("/api/attachments/{id}/thumbnail?key={access_key}"))
}

#[utoipa::path(
    get,
    path = "/api/attachments/{id}/thumbnail",
    params(
        ("id" = Uuid, description = "Attachment id"),
        ("key" = Uuid, Query, description = "The attachment's capability key, as in download_url")
    ),
    responses(
        (status = 200, description = "JPEG (or PNG when the image has transparency), longest edge 640 px"),
        (status = 404, description = "No such attachment for this key, not an image, or too large to thumbnail")
    )
)]
pub async fn download_thumbnail(
    State(state): State<SharedState>,
    Path(id): Path<Uuid>,
    Query(access): Query<AttachmentAccess>,
) -> Response<Body> {
    match thumbnail_bytes(&state, id, access.key).await {
        Ok(Some(bytes)) => image_response(bytes),
        Ok(None) => status(StatusCode::NOT_FOUND),
        Err(error) => {
            tracing::error!("attachment thumbnail failed: {error:#}");
            status(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

async fn thumbnail_bytes(
    state: &SharedState,
    id: Uuid,
    access_key: Uuid,
) -> anyhow::Result<Option<Vec<u8>>> {
    // Authorise first, every time — a cached file is never served on its own say-so.
    let Some(metadata) = state.attachment_metadata(id, access_key).await? else {
        return Ok(None);
    };
    if !thumbnailable(&metadata.mime_type) || metadata.size_bytes > MAX_SOURCE_BYTES {
        return Ok(None);
    }
    let store = state.attachment_store();
    let cached = cache_path(store.thumbnail_cache_dir(), id);
    if let Ok(bytes) = tokio::fs::read(&cached).await {
        return Ok(Some(bytes));
    }

    let _permit = RENDERS.acquire().await?;
    // Another request may have rendered it while this one waited for a permit.
    if let Ok(bytes) = tokio::fs::read(&cached).await {
        return Ok(Some(bytes));
    }
    let storage_key = metadata
        .storage_key
        .clone()
        .unwrap_or_else(|| id.simple().to_string());
    let mut reader = store
        .open_range(&storage_key, 0, metadata.size_bytes.max(0) as u64)
        .await?;
    let mut source = Vec::with_capacity(metadata.size_bytes.max(0) as usize);
    reader.read_to_end(&mut source).await?;
    let rendered = match tokio::task::spawn_blocking(move || render_thumbnail(&source)).await? {
        Ok(rendered) => rendered,
        Err(RenderError::Undecodable | RenderError::TooLarge) => return Ok(None),
    };
    write_cache(&cached, &rendered).await;
    Ok(Some(rendered))
}

fn cache_path(dir: PathBuf, id: Uuid) -> PathBuf {
    dir.join(id.simple().to_string())
}

/// Best effort: a failed cache write only means the next request renders again.
async fn write_cache(path: &std::path::Path, bytes: &[u8]) {
    let Some(dir) = path.parent() else { return };
    if tokio::fs::create_dir_all(dir).await.is_err() {
        return;
    }
    // Write beside, then rename, so a concurrent reader never sees a half-written file.
    let temporary = dir.join(format!(".{}.tmp", Uuid::new_v4().simple()));
    if tokio::fs::write(&temporary, bytes).await.is_ok()
        && tokio::fs::rename(&temporary, path).await.is_err()
    {
        let _ = tokio::fs::remove_file(&temporary).await;
    }
}

fn image_response(bytes: Vec<u8>) -> Response<Body> {
    let content_type = stored_content_type(&bytes);
    let length = bytes.len();
    let mut response = Response::new(Body::from(bytes));
    let headers = response.headers_mut();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static(content_type));
    headers.insert(CONTENT_LENGTH, HeaderValue::from(length));
    headers.insert(
        CACHE_CONTROL,
        HeaderValue::from_static("private, max-age=31536000, immutable"),
    );
    headers.insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    response
}

fn status(code: StatusCode) -> Response<Body> {
    let mut response = Response::new(Body::empty());
    *response.status_mut() = code;
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_raster_images_get_a_thumbnail_url() {
        let (id, key) = (Uuid::nil(), Uuid::max());
        assert_eq!(
            thumbnail_url(id, "image/jpeg", key).as_deref(),
            Some("/api/attachments/00000000-0000-0000-0000-000000000000/thumbnail?key=ffffffff-ffff-ffff-ffff-ffffffffffff")
        );
        for mime in [
            "image/PNG",
            "image/gif",
            "image/webp",
            "image/jpeg; charset=binary",
        ] {
            assert!(thumbnail_url(id, mime, key).is_some(), "{mime}");
        }
        for mime in [
            "image/svg+xml",
            "video/mp4",
            "audio/ogg",
            "application/pdf",
            "",
        ] {
            assert!(thumbnail_url(id, mime, key).is_none(), "{mime}");
        }
    }
}
