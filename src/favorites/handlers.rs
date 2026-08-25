use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use uuid::Uuid;

use crate::favorites::collaboration::FavoriteUpdateOutcome;
use crate::favorites::models::{
    AddFavoriteCollaboratorRequest, CreateFavoriteRequest, FavoriteCollaborator,
    FavoriteForwardResult, FavoriteItem, FavoriteMessagesRequest, ForwardFavoriteRequest,
    UpdateFavoriteRequest,
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
    put,
    path = "/api/favorites/{id}",
    params(("id" = Uuid, Path, description = "Favorite id")),
    request_body = UpdateFavoriteRequest,
    responses(
        (status = 200, description = "Favorite updated", body = FavoriteItem),
        (status = 400, description = "Invalid title, content, or version"),
        (status = 404, description = "Favorite not found or inaccessible"),
        (status = 409, description = "Favorite was changed by another editor")
    )
)]
pub async fn update_favorite(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(favorite_id): Path<Uuid>,
    Json(payload): Json<UpdateFavoriteRequest>,
) -> Result<Json<FavoriteItem>, StatusCode> {
    let user = current_user(&state, &headers).await?;
    let title = payload.title.trim();
    let content = payload.content.trim();
    let current = state
        .favorite_by_id(user.id, favorite_id)
        .await
        .map_err(internal_error)?
        .ok_or(StatusCode::NOT_FOUND)?;
    if payload.version <= 0
        || title.chars().count() > 120
        || content.chars().count() > 8_000
        || (current.kind == "manual" && title.is_empty() && content.is_empty())
    {
        return Err(StatusCode::BAD_REQUEST);
    }
    match state
        .update_favorite(user.id, favorite_id, payload.version, title, content)
        .await
        .map_err(internal_error)?
    {
        FavoriteUpdateOutcome::Updated(favorite) => Ok(Json(favorite)),
        FavoriteUpdateOutcome::Conflict => Err(StatusCode::CONFLICT),
        FavoriteUpdateOutcome::NotFound => Err(StatusCode::NOT_FOUND),
    }
}

#[utoipa::path(
    get,
    path = "/api/favorites/{id}/collaborators",
    params(("id" = Uuid, Path, description = "Favorite id")),
    responses(
        (status = 200, description = "Favorite collaborators", body = [FavoriteCollaborator]),
        (status = 404, description = "Favorite not found or inaccessible")
    )
)]
pub async fn list_collaborators(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(favorite_id): Path<Uuid>,
) -> Result<Json<Vec<FavoriteCollaborator>>, StatusCode> {
    let user = current_user(&state, &headers).await?;
    state
        .favorite_collaborators(user.id, favorite_id)
        .await
        .map_err(internal_error)?
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

#[utoipa::path(
    post,
    path = "/api/favorites/{id}/collaborators",
    params(("id" = Uuid, Path, description = "Favorite id")),
    request_body = AddFavoriteCollaboratorRequest,
    responses(
        (status = 201, description = "Collaborator added", body = FavoriteCollaborator),
        (status = 400, description = "Collaborator must be an accepted friend"),
        (status = 403, description = "Only the owner can add collaborators"),
        (status = 404, description = "Favorite not found")
    )
)]
pub async fn add_collaborator(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(favorite_id): Path<Uuid>,
    Json(payload): Json<AddFavoriteCollaboratorRequest>,
) -> Result<(StatusCode, Json<FavoriteCollaborator>), StatusCode> {
    let user = current_user(&state, &headers).await?;
    let favorite = state
        .favorite_by_id(user.id, favorite_id)
        .await
        .map_err(internal_error)?
        .ok_or(StatusCode::NOT_FOUND)?;
    if favorite.access != "owner" {
        return Err(StatusCode::FORBIDDEN);
    }
    state
        .add_favorite_collaborator(user.id, favorite_id, payload.user_id)
        .await
        .map_err(internal_error)?
        .map(|collaborator| (StatusCode::CREATED, Json(collaborator)))
        .ok_or(StatusCode::BAD_REQUEST)
}

#[utoipa::path(
    delete,
    path = "/api/favorites/{id}/collaborators/{user_id}",
    params(
        ("id" = Uuid, Path, description = "Favorite id"),
        ("user_id" = Uuid, Path, description = "Collaborator user id")
    ),
    responses(
        (status = 204, description = "Collaborator removed or left"),
        (status = 404, description = "Collaboration not found or not removable")
    )
)]
pub async fn remove_collaborator(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path((favorite_id, collaborator_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, StatusCode> {
    let user = current_user(&state, &headers).await?;
    state
        .remove_favorite_collaborator(user.id, favorite_id, collaborator_id)
        .await
        .map_err(internal_error)?
        .then_some(StatusCode::NO_CONTENT)
        .ok_or(StatusCode::NOT_FOUND)
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
        let Ok(_permit) = state.work_queue().message().await else {
            results.push(FavoriteForwardResult {
                favorite_id,
                target_room_id,
                forwarded_message_id: None,
                skipped_reason: Some("server busy; retry this forward".into()),
            });
            continue;
        };
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
