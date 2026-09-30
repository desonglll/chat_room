//! HTTP surface of forum topics (`docs/devlog/TG-204.md`, Frozen interface). Every handler
//! re-authorizes on each request: reads need an active membership, writes their right.

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use uuid::Uuid;

use super::model::{
    CreateTopicRequest, ForumToggleRequest, ForumTopic, ForumTopicList, TopicError,
    UpdateTopicRequest,
};
use super::store::TopicRow;
use crate::chats::membership_handlers::{reject_private_chat, require_permission, session_user};
use crate::chats::roster_handlers::require_reader;
use crate::chats::{CapabilityError, ChatCapabilityChange};
use crate::models::{Chat, User};
use crate::state::SharedState;

impl From<TopicError> for StatusCode {
    fn from(error: TopicError) -> Self {
        match error {
            TopicError::NotForum | TopicError::Conflict(_) => StatusCode::CONFLICT,
            TopicError::NotFound => StatusCode::NOT_FOUND,
            TopicError::Forbidden | TopicError::Closed => StatusCode::FORBIDDEN,
            TopicError::Invalid(_) => StatusCode::BAD_REQUEST,
            TopicError::Database(error) => {
                tracing::error!("forum topic query failed: {error}");
                StatusCode::INTERNAL_SERVER_ERROR
            }
        }
    }
}

pub(super) fn internal(error: sqlx::Error) -> StatusCode {
    StatusCode::from(TopicError::Database(error))
}

/// The viewer, checked as an active member of a forum chat.
pub(super) async fn forum_reader(
    state: &SharedState,
    room_id: Uuid,
    headers: &HeaderMap,
) -> Result<User, StatusCode> {
    reject_private_chat(state, room_id).await?;
    let user = session_user(state, headers).await?;
    require_reader(state, room_id, user.id).await?;
    match state.chat_is_forum(room_id).await.map_err(internal)? {
        None => Err(StatusCode::NOT_FOUND),
        Some(false) => Err(TopicError::NotForum.into()),
        Some(true) => Ok(user),
    }
}

pub(super) async fn topic_of(
    state: &SharedState,
    room_id: Uuid,
    topic_id: Uuid,
) -> Result<TopicRow, StatusCode> {
    state
        .ensure_general_topic(room_id)
        .await
        .map_err(internal)?;
    state
        .load_topic(room_id, topic_id)
        .await
        .map_err(internal)?
        .ok_or(StatusCode::NOT_FOUND)
}

pub(super) async fn is_admin(
    state: &SharedState,
    room_id: Uuid,
    user_id: Uuid,
) -> Result<bool, StatusCode> {
    state
        .is_topic_admin(room_id, user_id)
        .await
        .map_err(internal)
}

#[utoipa::path(put, path = "/api/chats/{id}/forum", params(("id" = Uuid, description = "Chat id")),
    request_body = ForumToggleRequest,
    responses((status = 200, description = "Forum mode changed (a group becomes a supergroup)", body = Chat),
        (status = 403, description = "Missing chat.info"), (status = 409, description = "This chat type cannot be a forum")))]
pub async fn put_forum(
    State(state): State<SharedState>,
    Path(room_id): Path<Uuid>,
    headers: HeaderMap,
    Json(request): Json<ForumToggleRequest>,
) -> Result<Json<Chat>, StatusCode> {
    reject_private_chat(&state, room_id).await?;
    let user = session_user(&state, &headers).await?;
    require_permission(&state, room_id, user.id, "chat.info").await?;
    let change = ChatCapabilityChange {
        is_forum: Some(request.enabled),
        ..Default::default()
    };
    let outcome = match state.apply_chat_capabilities(room_id, change).await {
        Ok(outcome) => outcome,
        Err(CapabilityError::NotFound) => return Err(StatusCode::NOT_FOUND),
        Err(CapabilityError::NotSupported(_)) => return Err(StatusCode::CONFLICT),
        Err(CapabilityError::Database(error)) => return Err(internal(error)),
    };
    if request.enabled {
        state
            .ensure_general_topic(room_id)
            .await
            .map_err(internal)?;
    }
    Ok(Json(outcome.chat))
}

#[utoipa::path(get, path = "/api/chats/{id}/topics", params(("id" = Uuid, description = "Chat id")),
    responses((status = 200, description = "The forum's topics as the viewer sees them", body = ForumTopicList),
        (status = 403, description = "Not an active member"), (status = 409, description = "Not a forum")))]
pub async fn list_topics(
    State(state): State<SharedState>,
    Path(room_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<ForumTopicList>, StatusCode> {
    let user = forum_reader(&state, room_id, &headers).await?;
    state
        .ensure_general_topic(room_id)
        .await
        .map_err(internal)?;
    let can_manage = is_admin(&state, room_id, user.id).await?;
    let can_create = state
        .has_chat_permission(room_id, user.id, "chat.topics")
        .await
        .map_err(internal)?;
    let rows = state.chat_topics(room_id).await.map_err(internal)?;
    let topics = state
        .topic_views(room_id, user.id, can_manage, rows)
        .await
        .map_err(internal)?;
    Ok(Json(ForumTopicList {
        chat_id: room_id,
        is_forum: true,
        can_create,
        can_manage,
        topics,
    }))
}

#[utoipa::path(post, path = "/api/chats/{id}/topics", params(("id" = Uuid, description = "Chat id")),
    request_body = CreateTopicRequest,
    responses((status = 201, description = "Topic created", body = ForumTopic),
        (status = 400, description = "Invalid title, icon or colour"), (status = 403, description = "Missing chat.topics")))]
pub async fn create_topic(
    State(state): State<SharedState>,
    Path(room_id): Path<Uuid>,
    headers: HeaderMap,
    Json(request): Json<CreateTopicRequest>,
) -> Result<(StatusCode, Json<ForumTopic>), StatusCode> {
    let user = forum_reader(&state, room_id, &headers).await?;
    require_permission(&state, room_id, user.id, "chat.topics").await?;
    let row = state
        .insert_topic(
            room_id,
            user.id,
            request.title,
            request.icon_emoji.unwrap_or_default(),
            request.icon_custom_emoji_id,
            request.icon_color,
        )
        .await?;
    let admin = is_admin(&state, room_id, user.id).await?;
    let view = state
        .single_topic_view(row, user.id, admin)
        .await
        .map_err(internal)?;
    state.announce_topic(room_id, view.summary()).await;
    Ok((StatusCode::CREATED, Json(view)))
}

#[utoipa::path(get, path = "/api/chats/{id}/topics/{topic_id}",
    params(("id" = Uuid, description = "Chat id"), ("topic_id" = Uuid, description = "Topic id")),
    responses((status = 200, description = "One topic", body = ForumTopic), (status = 404, description = "No such topic")))]
pub async fn get_topic(
    State(state): State<SharedState>,
    Path((room_id, topic_id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
) -> Result<Json<ForumTopic>, StatusCode> {
    let user = forum_reader(&state, room_id, &headers).await?;
    let row = topic_of(&state, room_id, topic_id).await?;
    let admin = is_admin(&state, room_id, user.id).await?;
    state
        .single_topic_view(row, user.id, admin)
        .await
        .map(Json)
        .map_err(internal)
}

#[utoipa::path(patch, path = "/api/chats/{id}/topics/{topic_id}",
    params(("id" = Uuid, description = "Chat id"), ("topic_id" = Uuid, description = "Topic id")),
    request_body = UpdateTopicRequest,
    responses((status = 200, description = "Topic changed", body = ForumTopic),
        (status = 403, description = "Neither the creator nor a topic administrator")))]
pub async fn update_topic(
    State(state): State<SharedState>,
    Path((room_id, topic_id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
    Json(request): Json<UpdateTopicRequest>,
) -> Result<Json<ForumTopic>, StatusCode> {
    let user = forum_reader(&state, room_id, &headers).await?;
    let row = topic_of(&state, room_id, topic_id).await?;
    let admin = is_admin(&state, room_id, user.id).await?;
    let creator = !row.is_general && row.creator_id == Some(user.id);
    if !admin && (!creator || request.needs_manager()) {
        return Err(StatusCode::FORBIDDEN);
    }
    let row = state.update_topic(&row, &request).await?;
    let view = state
        .single_topic_view(row, user.id, admin)
        .await
        .map_err(internal)?;
    state.announce_topic(room_id, view.summary()).await;
    Ok(Json(view))
}

#[utoipa::path(delete, path = "/api/chats/{id}/topics/{topic_id}",
    params(("id" = Uuid, description = "Chat id"), ("topic_id" = Uuid, description = "Topic id")),
    responses((status = 204, description = "Topic and its messages deleted"),
        (status = 403, description = "Not a topic administrator"), (status = 409, description = "General cannot be deleted")))]
pub async fn delete_topic(
    State(state): State<SharedState>,
    Path((room_id, topic_id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
) -> Result<StatusCode, StatusCode> {
    let user = forum_reader(&state, room_id, &headers).await?;
    let row = topic_of(&state, room_id, topic_id).await?;
    if !is_admin(&state, room_id, user.id).await? {
        return Err(StatusCode::FORBIDDEN);
    }
    if row.is_general {
        return Err(TopicError::Conflict("General cannot be deleted").into());
    }
    let attachments = state
        .delete_topic_with_messages(&row)
        .await
        .map_err(internal)?;
    for attachment_id in attachments {
        if let Err(error) = state
            .recompute_attachment_orphan_status(attachment_id)
            .await
        {
            tracing::warn!("recompute attachment orphan status failed: {error:#}");
        }
    }
    state.invalidate_message_cache(room_id).await;
    let mut summary = row.summary();
    summary.deleted = true;
    state.announce_topic(room_id, summary).await;
    Ok(StatusCode::NO_CONTENT)
}
