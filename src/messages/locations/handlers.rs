//! HTTP surface of TG-407 locations: send, move or stop a live location, and the chat's
//! current live locations for the merged map. Translation only; rules live in `mod.rs`.

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use chrono::{Duration, Utc};
use serde::Deserialize;
use utoipa::ToSchema;
use uuid::Uuid;

use super::{LiveLocationEntry, LiveUpdate, LocationPoint};
use crate::chats::membership_handlers::session_user;
use crate::models::{ChatMessage, StoredMessage};
use crate::realtime::protocol::stored_message_to_chat;
use crate::state::SharedState;

/// Telegram's choices are 15 min, 1 h and 8 h; any period in this range is accepted.
const LIVE_SECONDS: std::ops::RangeInclusive<i64> = 60..=86_400;

#[derive(Debug, Deserialize, ToSchema)]
pub struct SendLocationRequest {
    #[serde(flatten)]
    pub point: LocationPoint,
    /// A venue name; shown as the message text.
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub address: String,
    /// Share live for this many seconds (60 s – 24 h); absent = a static location.
    #[serde(default)]
    pub live_seconds: Option<i64>,
    #[serde(default)]
    pub reply_to: Option<Uuid>,
    /// TG-204: the forum topic; absent = General.
    #[serde(default)]
    pub topic_id: Option<Uuid>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct LiveLocationUpdate {
    #[serde(flatten)]
    pub point: LocationPoint,
}

fn internal(error: sqlx::Error) -> StatusCode {
    tracing::error!("location query failed: {error}");
    StatusCode::INTERNAL_SERVER_ERROR
}

#[utoipa::path(post, path = "/api/chats/{id}/location-messages", params(("id" = Uuid, description = "Chat id")),
    request_body = SendLocationRequest,
    responses((status = 201, description = "The location message (also broadcast)", body = StoredMessage),
        (status = 400, description = "Coordinates, accuracy, heading, live period or text out of range"),
        (status = 403, description = "May not send here (permission, closed topic, slow mode)")))]
pub async fn send_location(
    State(state): State<SharedState>,
    Path(room_id): Path<Uuid>,
    headers: HeaderMap,
    Json(request): Json<SendLocationRequest>,
) -> Result<(StatusCode, Json<StoredMessage>), StatusCode> {
    let sender = session_user(&state, &headers).await?;
    let title = request.title.trim();
    let address = request.address.trim();
    if !request.point.is_valid()
        || title.chars().count() > 128
        || address.chars().count() > 256
        || request
            .live_seconds
            .is_some_and(|seconds| !LIVE_SECONDS.contains(&seconds))
    {
        return Err(StatusCode::BAD_REQUEST);
    }
    if !state
        .has_chat_permission(room_id, sender.id, "message.send")
        .await
        .map_err(internal)?
    {
        return Err(StatusCode::FORBIDDEN);
    }
    let topic_id = state
        .resolve_post_topic(room_id, sender.id, request.topic_id)
        .await
        .map_err(StatusCode::from)?;
    let live_until = request
        .live_seconds
        .map(|seconds| Utc::now() + Duration::seconds(seconds));
    let display_name = state.resolve_display_name(room_id, &sender).await;
    let message_id = state
        .insert_location_message(
            room_id,
            sender.id,
            &display_name,
            &request.point,
            title,
            address,
            live_until,
            request.reply_to,
            topic_id,
        )
        .await
        .map_err(internal)?
        .ok_or(StatusCode::FORBIDDEN)?;
    state.invalidate_message_cache(room_id).await;
    let message = state
        .message_by_id(message_id, Some(sender.id))
        .await
        .map_err(internal)?
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
    state
        .broadcast(room_id, stored_message_to_chat(message.clone()))
        .await;
    Ok((StatusCode::CREATED, Json(message)))
}

async fn move_or_stop(
    state: SharedState,
    room_id: Uuid,
    message_id: Uuid,
    headers: HeaderMap,
    point: Option<LocationPoint>,
) -> Result<Json<super::MessageLocation>, StatusCode> {
    let sender = session_user(&state, &headers).await?;
    let stop = point.is_none();
    let point = match point {
        Some(point) if point.is_valid() => point,
        Some(_) => return Err(StatusCode::BAD_REQUEST),
        None => {
            // Stopping keeps the last point: read it back and write it unchanged.
            let entries = state.live_locations(room_id).await.map_err(internal)?;
            let current = entries
                .into_iter()
                .find(|entry| entry.message_id == message_id)
                .ok_or(StatusCode::NOT_FOUND)?;
            LocationPoint {
                latitude: current.location.latitude,
                longitude: current.location.longitude,
                accuracy_m: current.location.accuracy_m,
                heading: current.location.heading,
            }
        }
    };
    match state
        .update_live_location(room_id, sender.id, message_id, &point, stop)
        .await
        .map_err(internal)?
    {
        LiveUpdate::Updated(location) => {
            state.invalidate_message_cache(room_id).await;
            state
                .broadcast(
                    room_id,
                    ChatMessage::LocationUpdated {
                        message_id,
                        location: location.clone(),
                    },
                )
                .await;
            Ok(Json(location))
        }
        LiveUpdate::Ended => Err(StatusCode::CONFLICT),
        LiveUpdate::NotFound => Err(StatusCode::NOT_FOUND),
    }
}

#[utoipa::path(put, path = "/api/chats/{id}/live-locations/{message_id}",
    params(("id" = Uuid, description = "Chat id"), ("message_id" = Uuid, description = "Live location message")),
    request_body = LiveLocationUpdate,
    responses((status = 200, description = "The location now", body = super::MessageLocation),
        (status = 404, description = "Not your live location in this chat"),
        (status = 409, description = "Sharing has ended; no further points are accepted")))]
pub async fn update_live_location(
    State(state): State<SharedState>,
    Path((room_id, message_id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
    Json(request): Json<LiveLocationUpdate>,
) -> Result<Json<super::MessageLocation>, StatusCode> {
    move_or_stop(state, room_id, message_id, headers, Some(request.point)).await
}

#[utoipa::path(delete, path = "/api/chats/{id}/live-locations/{message_id}",
    params(("id" = Uuid, description = "Chat id"), ("message_id" = Uuid, description = "Live location message")),
    responses((status = 200, description = "Stopped; the last point stays", body = super::MessageLocation),
        (status = 404, description = "Not a live location of yours that is still running")))]
pub async fn stop_live_location(
    State(state): State<SharedState>,
    Path((room_id, message_id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
) -> Result<Json<super::MessageLocation>, StatusCode> {
    move_or_stop(state, room_id, message_id, headers, None).await
}

#[utoipa::path(get, path = "/api/chats/{id}/live-locations", params(("id" = Uuid, description = "Chat id")),
    responses((status = 200, description = "Locations being shared live now", body = Vec<LiveLocationEntry>),
        (status = 404, description = "Not a reader of this chat")))]
pub async fn list_live_locations(
    State(state): State<SharedState>,
    Path(room_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Vec<LiveLocationEntry>>, StatusCode> {
    let viewer = session_user(&state, &headers).await?;
    if !state
        .can_read_chat(room_id, viewer.id)
        .await
        .map_err(internal)?
    {
        return Err(StatusCode::NOT_FOUND);
    }
    state
        .live_locations(room_id)
        .await
        .map(Json)
        .map_err(internal)
}
