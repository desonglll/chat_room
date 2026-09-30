//! `GET /api/gifs/saved/:id/file?key=` — the account's saved copy, behind its capability
//! key like every attachment. Serves byte ranges: Safari will not play an MP4 whose server
//! answers a `Range` request with the whole body.

use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{
        header::{
            ACCEPT_RANGES, CACHE_CONTROL, CONTENT_DISPOSITION, CONTENT_LENGTH, CONTENT_RANGE,
            CONTENT_TYPE, RANGE,
        },
        HeaderMap, HeaderValue, StatusCode,
    },
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use tokio_util::io::ReaderStream;
use uuid::Uuid;

use crate::state::{with_pool, SharedState};
use crate::stickers::errors::StickerError;

#[derive(Deserialize)]
pub struct SavedGifAccess {
    key: Uuid,
}

/// One `bytes=` range of a `size`-byte body, inclusive; `Err` when unsatisfiable.
pub(crate) fn byte_range(header: Option<&str>, size: u64) -> Result<Option<(u64, u64)>, ()> {
    let Some(spec) = header.and_then(|value| value.trim().strip_prefix("bytes=")) else {
        return Ok(None);
    };
    if spec.contains(',') || size == 0 {
        return Err(());
    }
    let (start, end) = spec.split_once('-').ok_or(())?;
    let (start, end) = match (start.trim(), end.trim()) {
        ("", suffix) => {
            let suffix: u64 = suffix.parse().map_err(|_| ())?;
            if suffix == 0 {
                return Err(());
            }
            (size.saturating_sub(suffix), size - 1)
        }
        (start, "") => (start.parse().map_err(|_| ())?, size - 1),
        (start, end) => {
            let end: u64 = end.parse().map_err(|_| ())?;
            (start.parse().map_err(|_| ())?, end.min(size - 1))
        }
    };
    if start > end || start >= size {
        return Err(());
    }
    Ok(Some((start, end)))
}

pub async fn download_saved_gif(
    State(state): State<SharedState>,
    Path(id): Path<Uuid>,
    Query(access): Query<SavedGifAccess>,
    headers: HeaderMap,
) -> Response {
    let row: Result<Option<(String, i64, String)>, sqlx::Error> = with_pool!(state, |pool| {
        sqlx::query_as(
            "SELECT mime_type, size_bytes, storage_key FROM user_saved_gifs \
             WHERE id = $1 AND access_key = $2",
        )
        .bind(id)
        .bind(access.key)
        .fetch_optional(pool)
        .await
    });
    let (mime_type, size_bytes, storage_key) = match row {
        Ok(Some(row)) => row,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(error) => return StickerError::Database(error).into_response(),
    };
    let size = size_bytes.max(0) as u64;
    let range_header = headers.get(RANGE).and_then(|value| value.to_str().ok());
    let range = match byte_range(range_header, size) {
        Ok(range) => range,
        Err(()) => {
            let mut response = StatusCode::RANGE_NOT_SATISFIABLE.into_response();
            if let Ok(value) = HeaderValue::from_str(&format!("bytes */{size}")) {
                response.headers_mut().insert(CONTENT_RANGE, value);
            }
            return response;
        }
    };
    let (start, end) = range.unwrap_or((0, size.saturating_sub(1)));
    let length = if size == 0 { 0 } else { end - start + 1 };
    let reader = match state
        .attachment_store()
        .open_range(&storage_key, start, length)
        .await
    {
        Ok(reader) => reader,
        Err(error) => return StickerError::Storage(error).into_response(),
    };
    let mut response = Response::new(Body::from_stream(ReaderStream::new(reader)));
    let headers = response.headers_mut();
    headers.insert(
        CONTENT_TYPE,
        HeaderValue::from_str(&mime_type)
            .unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream")),
    );
    headers.insert(CONTENT_LENGTH, HeaderValue::from(length));
    headers.insert(ACCEPT_RANGES, HeaderValue::from_static("bytes"));
    headers.insert(
        CACHE_CONTROL,
        HeaderValue::from_static("private, max-age=31536000, immutable"),
    );
    headers.insert(CONTENT_DISPOSITION, HeaderValue::from_static("inline"));
    headers.insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    if range.is_some() {
        if let Ok(value) = HeaderValue::from_str(&format!("bytes {start}-{end}/{size}")) {
            headers.insert(CONTENT_RANGE, value);
        }
        *response.status_mut() = StatusCode::PARTIAL_CONTENT;
    }
    response
}

#[cfg(test)]
mod tests {
    use super::byte_range;

    #[test]
    fn parses_single_ranges() {
        assert_eq!(byte_range(None, 10), Ok(None));
        assert_eq!(byte_range(Some("bytes=2-5"), 10), Ok(Some((2, 5))));
        assert_eq!(byte_range(Some("bytes=7-"), 10), Ok(Some((7, 9))));
        assert_eq!(byte_range(Some("bytes=-3"), 10), Ok(Some((7, 9))));
        assert_eq!(byte_range(Some("bytes=0-99"), 10), Ok(Some((0, 9))));
        assert!(byte_range(Some("bytes=10-"), 10).is_err());
        assert!(byte_range(Some("bytes=0-1,3-4"), 10).is_err());
    }
}
