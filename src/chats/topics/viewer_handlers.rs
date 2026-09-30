//! The per-viewer HTTP surface of a forum topic: its message pages, the viewer's read
//! cursor and the viewer's mute. Split from `handlers` (file-size gate), same rules.

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use uuid::Uuid;

use super::handlers::{forum_reader, internal, is_admin, topic_of};
use super::model::{
    ForumTopic, TopicHistoryQuery, TopicNotificationsRequest, TopicReadRequest, TopicReadResult,
};
use super::reads::TopicMute;
use crate::chats::handlers::authorize_chat;
use crate::models::{Chat, StoredMessage};
use crate::state::SharedState;

/// The history endpoints also honour a chat password, exactly like `/messages`.
fn password_ok(state_chat: Option<Chat>, headers: &HeaderMap) -> Result<(), StatusCode> {
    let chat = state_chat.ok_or(StatusCode::NOT_FOUND)?;
    if !chat.has_password {
        return Ok(());
    }
    let supplied = headers
        .get("x-room-password")
        .and_then(|value| value.to_str().ok())
        .ok_or(StatusCode::UNAUTHORIZED)?;
    authorize_chat(&chat, Some(supplied))
        .then_some(())
        .ok_or(StatusCode::UNAUTHORIZED)
}

#[utoipa::path(get, path = "/api/chats/{id}/topics/{topic_id}/messages",
    params(("id" = Uuid, description = "Chat id"), ("topic_id" = Uuid, description = "Topic id"),
        ("limit" = Option<i64>, Query, description = "Messages to return (1-500)"),
        ("before" = Option<Uuid>, Query, description = "Message cursor, as on /messages")),
    responses((status = 200, description = "The topic's newest page, same order as /messages", body = Vec<StoredMessage>),
        (status = 400, description = "Unknown `before` cursor")))]
pub async fn list_topic_messages(
    State(state): State<SharedState>,
    Path((room_id, topic_id)): Path<(Uuid, Uuid)>,
    Query(query): Query<TopicHistoryQuery>,
    headers: HeaderMap,
) -> Result<Json<Vec<StoredMessage>>, StatusCode> {
    let user = forum_reader(&state, room_id, &headers).await?;
    password_ok(state.chat(room_id).await, &headers)?;
    let row = topic_of(&state, room_id, topic_id).await?;
    let before = match query.before {
        Some(message_id) => Some(
            state
                .topic_message_cursor(&row, message_id)
                .await
                .map_err(internal)?
                .ok_or(StatusCode::BAD_REQUEST)?,
        ),
        None => None,
    };
    state
        .topic_message_history(&row, query.limit.unwrap_or(100), before.as_ref(), user.id)
        .await
        .map(Json)
        .map_err(internal)
}

#[utoipa::path(get, path = "/api/chats/{id}/topics/{topic_id}/messages/{message_id}/context",
    params(("id" = Uuid, description = "Chat id"), ("topic_id" = Uuid, description = "Topic id"),
        ("message_id" = Uuid, description = "Target message"),
        ("limit" = Option<i64>, Query, description = "Messages to return (1-100)")),
    responses((status = 200, description = "Messages of the topic around the target", body = Vec<StoredMessage>),
        (status = 404, description = "The message is not in this topic")))]
pub async fn topic_message_context(
    State(state): State<SharedState>,
    Path((room_id, topic_id, message_id)): Path<(Uuid, Uuid, Uuid)>,
    Query(query): Query<TopicHistoryQuery>,
    headers: HeaderMap,
) -> Result<Json<Vec<StoredMessage>>, StatusCode> {
    let user = forum_reader(&state, room_id, &headers).await?;
    password_ok(state.chat(room_id).await, &headers)?;
    let row = topic_of(&state, room_id, topic_id).await?;
    state
        .topic_message_context(&row, message_id, query.limit.unwrap_or(50), user.id)
        .await
        .map_err(internal)?
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

#[utoipa::path(post, path = "/api/chats/{id}/topics/{topic_id}/read",
    params(("id" = Uuid, description = "Chat id"), ("topic_id" = Uuid, description = "Topic id")),
    request_body = TopicReadRequest,
    responses((status = 200, description = "Topic read cursor advanced", body = TopicReadResult),
        (status = 400, description = "The message is not in this topic")))]
pub async fn read_topic(
    State(state): State<SharedState>,
    Path((room_id, topic_id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
    Json(request): Json<TopicReadRequest>,
) -> Result<Json<TopicReadResult>, StatusCode> {
    let user = forum_reader(&state, room_id, &headers).await?;
    let row = topic_of(&state, room_id, topic_id).await?;
    let result = state
        .mark_topic_read(&row, user.id, request.message_id)
        .await?;
    Ok(Json(result))
}

#[utoipa::path(put, path = "/api/chats/{id}/topics/{topic_id}/notifications",
    params(("id" = Uuid, description = "Chat id"), ("topic_id" = Uuid, description = "Topic id")),
    request_body = TopicNotificationsRequest,
    responses((status = 200, description = "The viewer's mute for this topic changed", body = ForumTopic)))]
pub async fn put_topic_notifications(
    State(state): State<SharedState>,
    Path((room_id, topic_id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
    Json(request): Json<TopicNotificationsRequest>,
) -> Result<Json<ForumTopic>, StatusCode> {
    let user = forum_reader(&state, room_id, &headers).await?;
    let row = topic_of(&state, room_id, topic_id).await?;
    let mute = TopicMute {
        muted: request.muted,
        muted_until: request.muted.then_some(request.muted_until).flatten(),
    };
    state
        .set_topic_mute(row.id, user.id, mute)
        .await
        .map_err(internal)?;
    let admin = is_admin(&state, room_id, user.id).await?;
    state
        .single_topic_view(row, user.id, admin)
        .await
        .map(Json)
        .map_err(internal)
}
