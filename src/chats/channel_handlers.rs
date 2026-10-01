//! HTTP surface of channels (TG-202). Handlers translate protocol data and call
//! `chats::channels` / `chats::channel_views`; they hold no domain rule of their own.

use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    routing::{patch, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use super::channel_views::{record_channel_views, ViewError};
use super::channels::{Subscription, SubscriptionError};
use super::membership_handlers::{require_permission, session_user};
use crate::admin_system_lock::require_chat_unlocked;
use crate::handlers::authorize_chat;
use crate::models::{Chat, ChatMembership, ChatMessage, JoinChatRequest, MessageViewCount};
use crate::state::{AppState, SharedState};

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/api/chats/:id/subscription",
            post(subscribe).delete(unsubscribe),
        )
        .route("/api/chats/:id/channel", patch(update_channel))
        .route("/api/chats/:id/message-views", post(view_posts))
}

/// `PATCH /api/chats/:id/channel`.
#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateChannelRequest {
    pub signatures_enabled: bool,
}

/// `POST /api/chats/:id/message-views`.
#[derive(Debug, Deserialize, ToSchema)]
pub struct ViewPostsRequest {
    /// The channel posts on screen, at most 100.
    pub message_ids: Vec<Uuid>,
}

/// The posts' counts as stored now; the report itself arrives in the next batched
/// `message_views_updated` frame.
#[derive(Debug, Serialize, ToSchema)]
pub struct ViewPostsResponse {
    #[schema(value_type = Vec<Object>)]
    pub views: Vec<MessageViewCount>,
}

fn subscription_status(error: SubscriptionError) -> StatusCode {
    match error {
        SubscriptionError::NotAChannel => StatusCode::NOT_FOUND,
        SubscriptionError::Banned => StatusCode::FORBIDDEN,
        SubscriptionError::OwnerCannotLeave => StatusCode::CONFLICT,
        SubscriptionError::Database(error) => {
            tracing::error!("channel subscription failed: {error}");
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}

/// Subscribe to a channel. Silent: no frame announces a subscriber.
#[utoipa::path(
    post,
    path = "/api/chats/{id}/subscription",
    params(("id" = Uuid, description = "Channel id")),
    responses(
        (status = 200, description = "Subscribed", body = ChatMembership),
        (status = 202, description = "Waiting for approval", body = ChatMembership),
        (status = 401, description = "Wrong channel password"),
        (status = 403, description = "Banned from the channel"),
        (status = 404, description = "No such channel")
    )
)]
pub async fn subscribe(
    State(state): State<SharedState>,
    Path(room_id): Path<Uuid>,
    headers: HeaderMap,
    body: Option<Json<JoinChatRequest>>,
) -> Result<(StatusCode, Json<ChatMembership>), StatusCode> {
    let user = session_user(&state, &headers).await?;
    require_chat_unlocked(&state, room_id).await?;
    let chat = state.chat(room_id).await.ok_or(StatusCode::NOT_FOUND)?;
    let password = body
        .as_ref()
        .and_then(|Json(body)| body.password.as_deref());
    if !authorize_chat(&chat, password) {
        return Err(StatusCode::UNAUTHORIZED);
    }
    match state
        .subscribe_channel(room_id, user.id)
        .await
        .map_err(subscription_status)?
    {
        Subscription::Active(membership) => Ok((StatusCode::OK, Json(membership))),
        Subscription::Pending(membership) => Ok((StatusCode::ACCEPTED, Json(membership))),
    }
}

/// Unsubscribe. The owner cannot leave their own channel (409).
#[utoipa::path(
    delete,
    path = "/api/chats/{id}/subscription",
    params(("id" = Uuid, description = "Channel id")),
    responses(
        (status = 204, description = "Unsubscribed (or was not subscribed)"),
        (status = 404, description = "No such channel"),
        (status = 409, description = "The owner cannot unsubscribe")
    )
)]
pub async fn unsubscribe(
    State(state): State<SharedState>,
    Path(room_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<StatusCode, StatusCode> {
    let user = session_user(&state, &headers).await?;
    state
        .unsubscribe_channel(room_id, user.id)
        .await
        .map_err(subscription_status)?;
    Ok(StatusCode::NO_CONTENT)
}

/// Switch author signatures (`chat.info`). Broadcasts `chat_updated`.
#[utoipa::path(
    patch,
    path = "/api/chats/{id}/channel",
    params(("id" = Uuid, description = "Channel id")),
    request_body = UpdateChannelRequest,
    responses(
        (status = 200, description = "Updated channel", body = Chat),
        (status = 403, description = "Missing chat.info"),
        (status = 404, description = "No such channel")
    )
)]
pub async fn update_channel(
    State(state): State<SharedState>,
    Path(room_id): Path<Uuid>,
    headers: HeaderMap,
    Json(request): Json<UpdateChannelRequest>,
) -> Result<Json<Chat>, StatusCode> {
    let user = session_user(&state, &headers).await?;
    if !state.is_channel(room_id).await {
        return Err(StatusCode::NOT_FOUND);
    }
    require_chat_unlocked(&state, room_id).await?;
    require_permission(&state, room_id, user.id, "chat.info").await?;
    let mut chat = state
        .set_channel_signatures(room_id, request.signatures_enabled)
        .await
        .map_err(|error| {
            tracing::error!("update channel signatures failed: {error}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or(StatusCode::NOT_FOUND)?;
    chat.membership_status = None;
    chat.membership_role = None;
    state
        .broadcast(room_id, ChatMessage::ChatUpdated { chat: chat.clone() })
        .await;
    Ok(Json(chat))
}

/// Report channel posts the viewer has on screen. Each account counts once per post; counts
/// reach every subscriber in batched `message_views_updated` frames.
#[utoipa::path(
    post,
    path = "/api/chats/{id}/message-views",
    params(("id" = Uuid, description = "Channel id")),
    request_body = ViewPostsRequest,
    responses(
        (status = 200, description = "Current counts", body = ViewPostsResponse),
        (status = 400, description = "No ids, or more than 100"),
        (status = 404, description = "No such channel, or not a subscriber")
    )
)]
pub async fn view_posts(
    State(state): State<SharedState>,
    Path(room_id): Path<Uuid>,
    headers: HeaderMap,
    Json(request): Json<ViewPostsRequest>,
) -> Result<Json<ViewPostsResponse>, StatusCode> {
    let user = session_user(&state, &headers).await?;
    let views = record_channel_views(&state, room_id, user.id, &request.message_ids)
        .await
        .map_err(|error| match error {
            ViewError::NotFound => StatusCode::NOT_FOUND,
            ViewError::Invalid => StatusCode::BAD_REQUEST,
            ViewError::Database(error) => {
                tracing::error!("record channel views failed: {error}");
                StatusCode::INTERNAL_SERVER_ERROR
            }
        })?;
    Ok(Json(ViewPostsResponse { views }))
}
