use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::social::models::{
    FriendRequestAction, FriendRequestOutcome, FriendRequestPayload, FriendRequestView, SocialUser,
};
use crate::state::SharedState;
use crate::user_handlers::bearer_token;

#[derive(Deserialize)]
pub struct UserSearchQuery {
    q: String,
    #[serde(default = "default_limit")]
    limit: i64,
}

#[derive(Deserialize)]
pub struct FriendRequestsQuery {
    direction: String,
}

fn default_limit() -> i64 {
    20
}

async fn current_user_id(state: &SharedState, headers: &HeaderMap) -> Result<Uuid, StatusCode> {
    let token = bearer_token(headers)?;
    state
        .session_user(token)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .map(|user| user.id)
        .ok_or(StatusCode::UNAUTHORIZED)
}

#[utoipa::path(
    get,
    path = "/api/users/search",
    params(
        ("q" = String, Query, description = "Username or display-name prefix"),
        ("limit" = Option<i64>, Query, description = "Maximum number of results")
    ),
    responses(
        (status = 200, description = "Matching accounts", body = [SocialUser]),
        (status = 400, description = "Invalid query"),
        (status = 401, description = "Missing or expired session"),
        (status = 429, description = "Search rate limit exceeded")
    )
)]
pub async fn search_users(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Query(query): Query<UserSearchQuery>,
) -> Result<Json<Vec<SocialUser>>, StatusCode> {
    let user_id = current_user_id(&state, &headers).await?;
    let value = query.q.trim();
    if !(2..=64).contains(&value.chars().count()) || !(1..=50).contains(&query.limit) {
        return Err(StatusCode::BAD_REQUEST);
    }
    if !state.social_rate_limits.allow_search(user_id).await {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }
    state
        .search_social_users(user_id, value, query.limit)
        .await
        .map(Json)
        .map_err(|error| {
            tracing::error!("search users failed: {error}");
            StatusCode::INTERNAL_SERVER_ERROR
        })
}

#[utoipa::path(
    post,
    path = "/api/friend-requests",
    request_body = FriendRequestPayload,
    responses(
        (status = 201, description = "Friend request created"),
        (status = 200, description = "Existing request reused or accepted"),
        (status = 400, description = "Cannot request the current user"),
        (status = 404, description = "Target unavailable"),
        (status = 429, description = "Friend request rate limit exceeded")
    )
)]
pub async fn create_friend_request(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(payload): Json<FriendRequestPayload>,
) -> Result<StatusCode, StatusCode> {
    let user_id = current_user_id(&state, &headers).await?;
    if user_id == payload.user_id {
        return Err(StatusCode::BAD_REQUEST);
    }
    match state.send_friend_request(user_id, payload.user_id).await {
        Ok(FriendRequestOutcome::Created) => Ok(StatusCode::CREATED),
        Ok(FriendRequestOutcome::RateLimited) => Err(StatusCode::TOO_MANY_REQUESTS),
        Ok(_) => Ok(StatusCode::OK),
        Err(sqlx::Error::RowNotFound) => Err(StatusCode::NOT_FOUND),
        Err(error) => {
            tracing::error!("create friend request failed: {error}");
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/friend-requests",
    params(("direction" = String, Query, description = "incoming or outgoing")),
    responses(
        (status = 200, description = "Friend requests", body = [FriendRequestView]),
        (status = 400, description = "Invalid direction")
    )
)]
pub async fn list_friend_requests(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Query(query): Query<FriendRequestsQuery>,
) -> Result<Json<Vec<FriendRequestView>>, StatusCode> {
    if !matches!(query.direction.as_str(), "incoming" | "outgoing") {
        return Err(StatusCode::BAD_REQUEST);
    }
    let user_id = current_user_id(&state, &headers).await?;
    state
        .friend_requests(user_id, &query.direction)
        .await
        .map(Json)
        .map_err(|error| {
            tracing::error!("list friend requests failed: {error}");
            StatusCode::INTERNAL_SERVER_ERROR
        })
}

#[utoipa::path(
    patch,
    path = "/api/friend-requests/{user_id}",
    params(("user_id" = Uuid, Path, description = "Requesting user")),
    request_body = FriendRequestAction,
    responses(
        (status = 200, description = "Request accepted or declined"),
        (status = 404, description = "Incoming request not found")
    )
)]
pub async fn update_friend_request(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(requester_id): Path<Uuid>,
    Json(payload): Json<FriendRequestAction>,
) -> Result<StatusCode, StatusCode> {
    let accept = match payload.action.as_str() {
        "accept" => true,
        "decline" => false,
        _ => return Err(StatusCode::BAD_REQUEST),
    };
    let user_id = current_user_id(&state, &headers).await?;
    state
        .respond_friend_request(user_id, requester_id, accept)
        .await
        .map_err(|error| {
            tracing::error!("update friend request failed: {error}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .then_some(StatusCode::OK)
        .ok_or(StatusCode::NOT_FOUND)
}

#[utoipa::path(
    get,
    path = "/api/friends",
    responses((status = 200, description = "Accepted friends", body = [SocialUser]))
)]
pub async fn list_friends(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> Result<Json<Vec<SocialUser>>, StatusCode> {
    let user_id = current_user_id(&state, &headers).await?;
    state
        .friend_users(user_id)
        .await
        .map(Json)
        .map_err(|error| {
            tracing::error!("list friends failed: {error}");
            StatusCode::INTERNAL_SERVER_ERROR
        })
}

#[utoipa::path(
    delete,
    path = "/api/friend-requests/{user_id}",
    params(("user_id" = Uuid, Path, description = "Requested user")),
    responses(
        (status = 204, description = "Outgoing request canceled"),
        (status = 404, description = "Outgoing request not found")
    )
)]
pub async fn cancel_friend_request(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(target_id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    let user_id = current_user_id(&state, &headers).await?;
    state
        .cancel_friend_request(user_id, target_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .then_some(StatusCode::NO_CONTENT)
        .ok_or(StatusCode::NOT_FOUND)
}

#[utoipa::path(
    delete,
    path = "/api/friends/{user_id}",
    params(("user_id" = Uuid, Path, description = "Friend to remove")),
    responses((status = 204, description = "Friendship removed"))
)]
pub async fn delete_friend(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(target_id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    let user_id = current_user_id(&state, &headers).await?;
    if let Some(room_id) = state
        .remove_friend(user_id, target_id)
        .await
        .map_err(|error| {
            tracing::error!("delete friend failed: {error}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?
    {
        state
            .disconnect_room_member(room_id, user_id, "friendship removed")
            .await;
        state
            .disconnect_room_member(room_id, target_id, "friendship removed")
            .await;
    }
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    get,
    path = "/api/blocks",
    responses((status = 200, description = "Blocked users", body = [SocialUser]))
)]
pub async fn list_blocks(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> Result<Json<Vec<SocialUser>>, StatusCode> {
    let user_id = current_user_id(&state, &headers).await?;
    state
        .blocked_users(user_id)
        .await
        .map(Json)
        .map_err(|error| {
            tracing::error!("list blocks failed: {error}");
            StatusCode::INTERNAL_SERVER_ERROR
        })
}

#[utoipa::path(
    put,
    path = "/api/blocks/{user_id}",
    params(("user_id" = Uuid, Path, description = "User to block")),
    responses(
        (status = 204, description = "User blocked"),
        (status = 404, description = "User unavailable")
    )
)]
pub async fn block_user(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(target_id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    let user_id = current_user_id(&state, &headers).await?;
    if user_id == target_id {
        return Err(StatusCode::BAD_REQUEST);
    }
    let room_id = state
        .block_user(user_id, target_id)
        .await
        .map_err(|error| match error {
            sqlx::Error::RowNotFound => StatusCode::NOT_FOUND,
            other => {
                tracing::error!("block user failed: {other}");
                StatusCode::INTERNAL_SERVER_ERROR
            }
        })?;
    if let Some(room_id) = room_id {
        state
            .disconnect_room_member(room_id, user_id, "user blocked")
            .await;
        state
            .disconnect_room_member(room_id, target_id, "user blocked")
            .await;
    }
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    delete,
    path = "/api/blocks/{user_id}",
    params(("user_id" = Uuid, Path, description = "User to unblock")),
    responses((status = 204, description = "User unblocked"))
)]
pub async fn unblock_user(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(target_id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    let user_id = current_user_id(&state, &headers).await?;
    state
        .unblock_user(user_id, target_id)
        .await
        .map_err(|error| {
            tracing::error!("unblock user failed: {error}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(StatusCode::NO_CONTENT)
}
