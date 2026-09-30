//! HTTP translation for the GIF domain: authenticate, parse, call one `AppState` method.

use axum::{
    body::Bytes,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use uuid::Uuid;

use super::models::{
    RecentGif, RecentGifQuery, SaveGifRequest, SavedGif, SendGifRequest, UploadGifQuery,
};
use super::send::{GifPlacement, GifSource};
use crate::attachment_handlers::authorize_upload;
use crate::models::StoredMessage;
use crate::realtime::protocol::stored_message_to_chat;
use crate::state::SharedState;
use crate::stickers::errors::StickerError;
use crate::stickers::handlers::session_account;

pub async fn list_saved(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> Result<Json<Vec<SavedGif>>, StickerError> {
    let user = session_account(&state, &headers).await?;
    Ok(Json(state.saved_gifs(user.id).await?))
}

/// 201 when newly saved, 200 when it was already saved (and moved to the front).
pub async fn save(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(request): Json<SaveGifRequest>,
) -> Result<(StatusCode, Json<SavedGif>), StickerError> {
    let user = session_account(&state, &headers).await?;
    let (saved, created) = state.save_gif(user.id, request.message_id).await?;
    let status = if created {
        StatusCode::CREATED
    } else {
        StatusCode::OK
    };
    Ok((status, Json(saved)))
}

pub async fn remove(
    State(state): State<SharedState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<StatusCode, StickerError> {
    let user = session_account(&state, &headers).await?;
    if state.remove_saved_gif(user.id, id).await? {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(StickerError::NotFound("saved_gif_not_found"))
    }
}

pub async fn recent(
    State(state): State<SharedState>,
    Query(query): Query<RecentGifQuery>,
    headers: HeaderMap,
) -> Result<Json<Vec<RecentGif>>, StickerError> {
    let user = session_account(&state, &headers).await?;
    Ok(Json(state.recent_gifs(user.id, query.limit).await?))
}

/// `POST /api/chats/:id/gif-messages` — a saved GIF or a readable GIF message, by reference.
pub async fn send(
    State(state): State<SharedState>,
    Path(room_id): Path<Uuid>,
    headers: HeaderMap,
    Json(request): Json<SendGifRequest>,
) -> Result<(StatusCode, Json<StoredMessage>), StickerError> {
    let source = match (request.saved_gif_id, request.message_id) {
        (Some(id), None) => GifSource::Saved(id),
        (None, Some(id)) => GifSource::Message(id),
        _ => return Err(StickerError::Invalid("exactly_one_source")),
    };
    deliver(
        &state,
        room_id,
        &headers,
        source,
        request.reply_to,
        request.topic_id,
        request.client_message_id,
    )
    .await
}

/// `POST /api/chats/:id/gif-messages/upload` — the raw file as the request body.
pub async fn upload(
    State(state): State<SharedState>,
    Path(room_id): Path<Uuid>,
    Query(query): Query<UploadGifQuery>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<(StatusCode, Json<StoredMessage>), StickerError> {
    if body.is_empty() {
        return Err(StickerError::Invalid("empty_file"));
    }
    deliver(
        &state,
        room_id,
        &headers,
        GifSource::Upload(body.to_vec()),
        query.reply_to,
        query.topic_id,
        query.client_message_id,
    )
    .await
}

/// Authorized like an attachment upload (chat exists, chat password, session, the full
/// `message.send_sticker` decision), then persisted and broadcast like one.
async fn deliver(
    state: &SharedState,
    room_id: Uuid,
    headers: &HeaderMap,
    source: GifSource,
    reply_to: Option<Uuid>,
    topic_id: Option<Uuid>,
    client_message_id: Option<Uuid>,
) -> Result<(StatusCode, Json<StoredMessage>), StickerError> {
    // Telegram grants stickers and GIFs together ("Send Stickers & GIFs").
    let (chat, user) = authorize_upload(state, room_id, headers, "message.send_sticker").await?;
    let _permit = state
        .work_queue()
        .message()
        .await
        .map_err(|_| StickerError::Unavailable)?;
    // TG-204: the same topic gate as the sticker path.
    let topic_id = state
        .resolve_post_topic(chat.id, user.id, topic_id)
        .await
        .map_err(|error| match error {
            crate::chats::TopicError::NotFound => StickerError::NotFound("topic_not_found"),
            crate::chats::TopicError::Closed | crate::chats::TopicError::Forbidden => {
                StickerError::Forbidden
            }
            crate::chats::TopicError::Database(error) => StickerError::Database(error),
            _ => StickerError::Invalid("topic_id"),
        })?;
    let display_name = state.resolve_display_name(chat.id, &user).await;
    let sent = state
        .send_gif_message(
            chat.id,
            &user,
            &display_name,
            source,
            GifPlacement { reply_to, topic_id },
            client_message_id,
        )
        .await?;
    if !sent.inserted {
        return Ok((StatusCode::OK, Json(sent.message)));
    }
    state
        .broadcast(chat.id, stored_message_to_chat(sent.message.clone()))
        .await;
    Ok((StatusCode::CREATED, Json(sent.message)))
}
