//! `POST /api/chats/:id/sticker-messages`.
//!
//! Authorized by the attachment upload check (chat exists, chat password, session,
//! `message.send` through the full chat authorization decision), then persisted under the
//! same in-transaction membership re-check, cache invalidation and broadcast as an
//! attachment upload — the WebSocket outbound cursor and history see it like any message.

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use uuid::Uuid;

use crate::attachment_handlers::authorize_upload;
use crate::models::StoredMessage;
use crate::realtime::protocol::stored_message_to_chat;
use crate::state::SharedState;
use crate::stickers::errors::StickerError;
use crate::stickers::models::SendStickerRequest;

pub async fn send_sticker(
    State(state): State<SharedState>,
    Path(room_id): Path<Uuid>,
    headers: HeaderMap,
    Json(request): Json<SendStickerRequest>,
) -> Result<(StatusCode, Json<StoredMessage>), StickerError> {
    let (chat, user) = authorize_upload(&state, room_id, &headers).await?;
    let _permit = state
        .work_queue()
        .message()
        .await
        .map_err(|_| StickerError::Unavailable)?;
    let display_name = state.resolve_display_name(chat.id, &user).await;
    let sent = state
        .send_sticker_message(chat.id, &user, &display_name, &request)
        .await?;
    if !sent.inserted {
        return Ok((StatusCode::OK, Json(sent.message)));
    }
    state
        .broadcast(chat.id, stored_message_to_chat(sent.message.clone()))
        .await;
    Ok((StatusCode::CREATED, Json(sent.message)))
}
