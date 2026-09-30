//! `/api/stickers/*`: the caller's installed sets, recents, favorites and emoji search.

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use serde::Deserialize;
use uuid::Uuid;

use super::session_account;
use crate::state::SharedState;
use crate::stickers::errors::StickerError;
use crate::stickers::models::{
    ArchiveStickerSetRequest, InstalledStickerSets, ReorderStickerSetsRequest, Sticker,
};

#[derive(Deserialize)]
pub struct StickerSearch {
    emoji: String,
}

pub async fn list_installed(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> Result<Json<InstalledStickerSets>, StickerError> {
    let user = session_account(&state, &headers).await?;
    Ok(Json(state.installed_sticker_sets(user.id).await?))
}

pub async fn reorder(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(request): Json<ReorderStickerSetsRequest>,
) -> Result<Json<InstalledStickerSets>, StickerError> {
    let user = session_account(&state, &headers).await?;
    Ok(Json(
        state.reorder_sticker_sets(user.id, request.set_ids).await?,
    ))
}

pub async fn install(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(set_id): Path<Uuid>,
) -> Result<Json<InstalledStickerSets>, StickerError> {
    let user = session_account(&state, &headers).await?;
    Ok(Json(state.install_sticker_set(user.id, set_id).await?))
}

pub async fn archive(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(set_id): Path<Uuid>,
    Json(request): Json<ArchiveStickerSetRequest>,
) -> Result<Json<InstalledStickerSets>, StickerError> {
    let user = session_account(&state, &headers).await?;
    Ok(Json(
        state
            .archive_sticker_set(user.id, set_id, request.archived)
            .await?,
    ))
}

pub async fn uninstall(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(set_id): Path<Uuid>,
) -> Result<Json<InstalledStickerSets>, StickerError> {
    let user = session_account(&state, &headers).await?;
    Ok(Json(state.uninstall_sticker_set(user.id, set_id).await?))
}

pub async fn recent(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> Result<Json<Vec<Sticker>>, StickerError> {
    let user = session_account(&state, &headers).await?;
    Ok(Json(state.recent_stickers(user.id).await?))
}

pub async fn remove_recent(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(sticker_id): Path<Uuid>,
) -> Result<StatusCode, StickerError> {
    let user = session_account(&state, &headers).await?;
    state.remove_recent_sticker(user.id, sticker_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn favorites(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> Result<Json<Vec<Sticker>>, StickerError> {
    let user = session_account(&state, &headers).await?;
    Ok(Json(state.favorite_stickers(user.id).await?))
}

pub async fn add_favorite(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(sticker_id): Path<Uuid>,
) -> Result<StatusCode, StickerError> {
    let user = session_account(&state, &headers).await?;
    state.favorite_sticker(user.id, sticker_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn remove_favorite(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(sticker_id): Path<Uuid>,
) -> Result<StatusCode, StickerError> {
    let user = session_account(&state, &headers).await?;
    state.unfavorite_sticker(user.id, sticker_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn search(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Query(query): Query<StickerSearch>,
) -> Result<Json<Vec<Sticker>>, StickerError> {
    let user = session_account(&state, &headers).await?;
    if query.emoji.trim().is_empty() {
        return Err(StickerError::Invalid("invalid_emoji"));
    }
    Ok(Json(state.search_stickers(user.id, &query.emoji).await?))
}
