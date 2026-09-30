//! Sticker bytes: committing a validated upload to the attachment object store, and
//! serving the set-catalogue copy behind its capability key.

use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{
        header::{CACHE_CONTROL, CONTENT_DISPOSITION, CONTENT_LENGTH, CONTENT_TYPE},
        HeaderValue, StatusCode,
    },
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use tokio_util::io::ReaderStream;
use uuid::Uuid;

use super::errors::StickerError;
use super::sets::StoredStickerFile;
use super::validation::ValidatedSticker;
use crate::state::{with_pool, AppState, SharedState};

#[derive(Deserialize)]
pub struct StickerFileAccess {
    key: Uuid,
}

impl AppState {
    /// Content-addressed like attachments: the object key is the SHA-256 of the bytes, and
    /// identical bytes (another sticker, or an ordinary attachment) are stored once.
    pub(crate) async fn store_sticker_file(
        &self,
        bytes: &[u8],
        validated: ValidatedSticker,
    ) -> Result<StoredStickerFile, StickerError> {
        let store = self.attachment_store();
        let mut staged = store.begin().await.map_err(StickerError::Storage)?;
        staged.write(bytes).await.map_err(StickerError::Storage)?;
        let content_hash = store
            .hash_staged(&mut staged)
            .await
            .map_err(StickerError::Storage)?;
        let size_bytes = staged.size();
        let _guard = self.content_hash_locks().lock(&content_hash).await;
        if store
            .exists(&content_hash)
            .await
            .map_err(StickerError::Storage)?
        {
            drop(staged);
        } else {
            store
                .commit(staged, &content_hash)
                .await
                .map_err(StickerError::Storage)?;
        }
        Ok(StoredStickerFile {
            validated,
            size_bytes,
            storage_key: content_hash.clone(),
            content_hash,
        })
    }
}

/// `GET /api/stickers/:id/file?key=` — the catalogue copy of a live sticker.
pub async fn download_sticker_file(
    State(state): State<SharedState>,
    Path(id): Path<Uuid>,
    Query(access): Query<StickerFileAccess>,
) -> Response {
    let row: Result<Option<(String, i64, String)>, sqlx::Error> = with_pool!(state, |pool| {
        sqlx::query_as(
            "SELECT mime_type, size_bytes, storage_key FROM stickers \
             WHERE id = $1 AND access_key = $2 AND removed_at IS NULL",
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
    let reader = match state
        .attachment_store()
        .open_range(&storage_key, 0, size_bytes as u64)
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
    headers.insert(CONTENT_LENGTH, HeaderValue::from(size_bytes));
    headers.insert(
        CACHE_CONTROL,
        HeaderValue::from_static("private, max-age=31536000, immutable"),
    );
    headers.insert(CONTENT_DISPOSITION, HeaderValue::from_static("inline"));
    headers.insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    response
}
