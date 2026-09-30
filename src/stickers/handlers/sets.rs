//! `/api/sticker-sets/*`: create a set, read it by short name, add and remove stickers.

use axum::{
    extract::{Multipart, Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use uuid::Uuid;

use super::session_account;
use crate::state::SharedState;
use crate::stickers::errors::StickerError;
use crate::stickers::models::{CreateStickerSetRequest, Sticker, StickerFormat, StickerSet};
use crate::stickers::sets::parse_emoji_list;
use crate::stickers::validation::{validate_sticker_file, StickerRejection};

/// The largest file any format accepts; anything longer is refused while streaming.
const MAX_UPLOAD_BYTES: usize = StickerFormat::Webp.max_bytes();

pub async fn create_set(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(request): Json<CreateStickerSetRequest>,
) -> Result<(StatusCode, Json<StickerSet>), StickerError> {
    let user = session_account(&state, &headers).await?;
    let set = state.create_sticker_set(user.id, &request).await?;
    Ok((StatusCode::CREATED, Json(set)))
}

pub async fn get_set(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(short_name): Path<String>,
) -> Result<Json<StickerSet>, StickerError> {
    let user = session_account(&state, &headers).await?;
    state
        .sticker_set_by_short_name(&short_name, user.id)
        .await?
        .map(Json)
        .ok_or(StickerError::NotFound("sticker_set_not_found"))
}

/// Multipart: `file` (the sticker) and `emoji` (space-separated, 1–20).
pub async fn add_sticker(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(short_name): Path<String>,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<Sticker>), StickerError> {
    let user = session_account(&state, &headers).await?;
    let set = state.owned_sticker_set(&short_name, user.id).await?;
    let mut file: Option<Vec<u8>> = None;
    let mut emoji = None;
    while let Some(mut field) = multipart
        .next_field()
        .await
        .map_err(|error| StickerError::from(error.status()))?
    {
        match field.name() {
            Some("file") if file.is_none() => {
                let mut bytes = Vec::new();
                while let Some(chunk) = field
                    .chunk()
                    .await
                    .map_err(|error| StickerError::from(error.status()))?
                {
                    if bytes.len() + chunk.len() > MAX_UPLOAD_BYTES {
                        return Err(StickerRejection::FileTooLarge.into());
                    }
                    bytes.extend_from_slice(&chunk);
                }
                file = Some(bytes);
            }
            Some("emoji") => {
                let text = field
                    .text()
                    .await
                    .map_err(|_| StickerError::Invalid("invalid_emoji"))?;
                emoji =
                    Some(parse_emoji_list(&text).ok_or(StickerError::Invalid("invalid_emoji"))?);
            }
            _ => {}
        }
    }
    let file = file.ok_or(StickerRejection::UnsupportedFormat)?;
    let emojis = emoji.ok_or(StickerError::Invalid("invalid_emoji"))?;
    let validated = validate_sticker_file(&file, set.set_type())?;
    let _permit = state
        .work_queue()
        .upload()
        .await
        .map_err(|_| StickerError::Unavailable)?;
    let stored = state.store_sticker_file(&file, validated).await?;
    let sticker = state.add_sticker(&set, &emojis, stored).await?;
    Ok((StatusCode::CREATED, Json(sticker)))
}

pub async fn remove_sticker(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path((short_name, sticker_id)): Path<(String, Uuid)>,
) -> Result<StatusCode, StickerError> {
    let user = session_account(&state, &headers).await?;
    let set = state.owned_sticker_set(&short_name, user.id).await?;
    state.remove_sticker(&set, sticker_id).await?;
    Ok(StatusCode::NO_CONTENT)
}
