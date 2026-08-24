use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use uuid::Uuid;

use crate::favorites::models::{
    CreateFavoriteRequest, FavoriteForwardResult, FavoriteItem, FavoriteMessagesRequest,
    ForwardFavoriteRequest,
};
use crate::models::User;
use crate::realtime::protocol::stored_message_to_chat;
use crate::state::SharedState;
use crate::user_handlers::bearer_token;

async fn current_user(state: &SharedState, headers: &HeaderMap) -> Result<User, StatusCode> {
    let token = bearer_token(headers)?;
    state
        .session_user(token)
        .await
        .map_err(internal_error)?
        .ok_or(StatusCode::UNAUTHORIZED)
}

#[utoipa::path(
    get,
    path = "/api/favorites",
    responses(
        (status = 200, description = "Current user's favorites", body = [FavoriteItem]),
        (status = 401, description = "Missing or expired session")
    )
)]
pub async fn list_favorites(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> Result<Json<Vec<FavoriteItem>>, StatusCode> {
    let user = current_user(&state, &headers).await?;
    state
        .favorites(user.id)
        .await
        .map(Json)
        .map_err(internal_error)
}

#[utoipa::path(
    post,
    path = "/api/favorites",
    request_body = CreateFavoriteRequest,
    responses(
        (status = 201, description = "Manual favorite created", body = FavoriteItem),
        (status = 400, description = "Invalid title or content")
    )
)]
pub async fn create_favorite(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(payload): Json<CreateFavoriteRequest>,
) -> Result<(StatusCode, Json<FavoriteItem>), StatusCode> {
    let user = current_user(&state, &headers).await?;
    let title = payload.title.trim();
    let content = payload.content.trim();
    if title.chars().count() > 120
        || content.chars().count() > 8_000
        || (title.is_empty() && content.is_empty())
    {
        return Err(StatusCode::BAD_REQUEST);
    }
    state
        .create_manual_favorite(user.id, title, content)
        .await
        .map(|favorite| (StatusCode::CREATED, Json(favorite)))
        .map_err(internal_error)
}

#[utoipa::path(
    post,
    path = "/api/favorites/messages",
    request_body = FavoriteMessagesRequest,
    responses(
        (status = 200, description = "Messages added to favorites", body = [FavoriteItem]),
        (status = 400, description = "Invalid message batch")
    )
)]
pub async fn favorite_messages(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(payload): Json<FavoriteMessagesRequest>,
) -> Result<Json<Vec<FavoriteItem>>, StatusCode> {
    let user = current_user(&state, &headers).await?;
    if payload.message_ids.is_empty() || payload.message_ids.len() > 100 {
        return Err(StatusCode::BAD_REQUEST);
    }
    let mut favorites = Vec::with_capacity(payload.message_ids.len());
    for message_id in payload.message_ids {
        match state.favorite_message(user.id, message_id).await {
            Ok(Some(favorite)) => favorites.push(favorite),
            Ok(None) => {}
            Err(error) => return Err(internal_error(error)),
        }
    }
    Ok(Json(favorites))
}

#[utoipa::path(
    delete,
    path = "/api/favorites/{id}",
    params(("id" = Uuid, Path, description = "Favorite id")),
    responses(
        (status = 204, description = "Favorite deleted"),
        (status = 404, description = "Favorite not found")
    )
)]
pub async fn delete_favorite(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(favorite_id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    let user = current_user(&state, &headers).await?;
    let (deleted, _) = state
        .delete_favorite(user.id, favorite_id)
        .await
        .map_err(internal_error)?;
    deleted
        .then_some(StatusCode::NO_CONTENT)
        .ok_or(StatusCode::NOT_FOUND)
}

#[utoipa::path(
    post,
    path = "/api/favorites/{id}/forward",
    params(("id" = Uuid, Path, description = "Favorite id")),
    request_body = ForwardFavoriteRequest,
    responses(
        (status = 200, description = "Per-room forward outcome", body = [FavoriteForwardResult]),
        (status = 400, description = "Invalid target-room batch"),
        (status = 404, description = "Favorite not found")
    )
)]
pub async fn forward_favorite(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(favorite_id): Path<Uuid>,
    Json(payload): Json<ForwardFavoriteRequest>,
) -> Result<Json<Vec<FavoriteForwardResult>>, StatusCode> {
    let user = current_user(&state, &headers).await?;
    if payload.target_room_ids.is_empty() || payload.target_room_ids.len() > 50 {
        return Err(StatusCode::BAD_REQUEST);
    }
    if state
        .favorite_by_id(user.id, favorite_id)
        .await
        .map_err(internal_error)?
        .is_none()
    {
        return Err(StatusCode::NOT_FOUND);
    }
    let mut results = Vec::with_capacity(payload.target_room_ids.len());
    for target_room_id in payload.target_room_ids {
        match state
            .forward_favorite(favorite_id, target_room_id, &user)
            .await
        {
            Ok(Some(message)) => {
                let forwarded_message_id = message.id;
                state
                    .broadcast(target_room_id, stored_message_to_chat(message))
                    .await;
                results.push(FavoriteForwardResult {
                    favorite_id,
                    target_room_id,
                    forwarded_message_id: Some(forwarded_message_id),
                    skipped_reason: None,
                });
            }
            Ok(None) => results.push(FavoriteForwardResult {
                favorite_id,
                target_room_id,
                forwarded_message_id: None,
                skipped_reason: Some("cannot send to the target room".into()),
            }),
            Err(error) => {
                tracing::error!(%favorite_id, %target_room_id, "forward favorite failed: {error}");
                results.push(FavoriteForwardResult {
                    favorite_id,
                    target_room_id,
                    forwarded_message_id: None,
                    skipped_reason: Some("forward failed".into()),
                });
            }
        }
    }
    Ok(Json(results))
}

fn internal_error(error: sqlx::Error) -> StatusCode {
    tracing::error!("favorite operation failed: {error}");
    StatusCode::INTERNAL_SERVER_ERROR
}
