//! HTTP translation for custom emoji and emoji status. Every route is bearer-authenticated.

use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use serde::Deserialize;
use uuid::Uuid;

use super::models::{CustomEmoji, EmojiStatus, SetEmojiStatusRequest, MAX_LOOKUP_IDS};
use crate::state::SharedState;
use crate::stickers::errors::StickerError;
use crate::stickers::handlers::session_account;
use crate::stickers::models::InstalledStickerSets;

/// `?ids=a,b,c` — comma-separated UUIDs, deduplicated in order, at most [`MAX_LOOKUP_IDS`].
#[derive(Deserialize)]
pub struct IdList {
    #[serde(default)]
    ids: String,
}

impl IdList {
    fn parse(&self) -> Result<Vec<Uuid>, StickerError> {
        let mut ids: Vec<Uuid> = Vec::new();
        for token in self.ids.split(',').map(str::trim).filter(|t| !t.is_empty()) {
            let id = Uuid::parse_str(token).map_err(|_| StickerError::Invalid("invalid_id"))?;
            if !ids.contains(&id) {
                ids.push(id);
            }
        }
        if ids.len() > MAX_LOOKUP_IDS {
            return Err(StickerError::Invalid("too_many_ids"));
        }
        Ok(ids)
    }
}

/// `GET /api/custom-emoji?ids=` — the live custom emoji among `ids`, in request order.
pub async fn resolve(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Query(query): Query<IdList>,
) -> Result<Json<Vec<CustomEmoji>>, StickerError> {
    session_account(&state, &headers).await?;
    let ids = query.parse()?;
    let mut found = state.resolve_custom_emoji(&ids).await?;
    Ok(Json(ids.iter().filter_map(|id| found.remove(id)).collect()))
}

/// `GET /api/custom-emoji/installed`.
pub async fn installed(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> Result<Json<InstalledStickerSets>, StickerError> {
    let user = session_account(&state, &headers).await?;
    Ok(Json(state.installed_custom_emoji_sets(user.id).await?))
}

/// `PUT /api/users/me/emoji-status`.
pub async fn set_status(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(request): Json<SetEmojiStatusRequest>,
) -> Result<Json<EmojiStatus>, StickerError> {
    let user = session_account(&state, &headers).await?;
    Ok(Json(state.set_emoji_status(user.id, &request).await?))
}

/// `DELETE /api/users/me/emoji-status`.
pub async fn clear_status(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> Result<StatusCode, StickerError> {
    let user = session_account(&state, &headers).await?;
    state.clear_emoji_status(user.id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// `GET /api/users/emoji-statuses?ids=`.
pub async fn statuses(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Query(query): Query<IdList>,
) -> Result<Json<Vec<EmojiStatus>>, StickerError> {
    let user = session_account(&state, &headers).await?;
    let ids = query.parse()?;
    Ok(Json(state.emoji_statuses(user.id, &ids).await?))
}
