//! TG-403 albums (media groups): 2-10 photos/videos sent as one unit.
//!
//! An album is not a new kind of row. It is N ordinary media messages sharing
//! `messages.grouped_id`, so every existing read path (history, search, unread counts,
//! previews, reactions, recall, the media viewer) keeps working per item, and clients draw
//! consecutive members as one mosaic bubble.
//!
//! Sending is all-or-nothing in two phases:
//! 1. every upload session is validated, then its bytes are published under their content
//!    address (`promote.rs`) — no database row exists yet;
//! 2. one transaction claims every session and inserts every attachment and message
//!    (`store.rs`). Any failure rolls the whole album back; nothing is broadcast until commit.
//!
//! The HTTP handlers (`handlers.rs`) only translate; the rules live here.

mod forward;
pub mod handlers;
pub mod model;
mod promote;
mod store;

use std::sync::Arc;

use axum::{
    routing::{delete, post},
    Router,
};
use chrono::Utc;
use uuid::Uuid;

use crate::models::{ChatMessage, User};
use crate::realtime::protocol::stored_message_to_chat;
use crate::state::{AppState, SharedState};

pub use forward::ForwardPlan;
use model::{
    is_album_media, AlbumError, RecallAlbumResponse, RecalledAlbumItem, SendAlbumRequest,
    SendAlbumResponse, MAX_ALBUM_ITEMS, MAX_CAPTION_CHARS, MIN_ALBUM_ITEMS,
};
use store::NewAlbum;

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/chats/:id/albums", post(handlers::send))
        .route(
            "/api/chats/:id/albums/:grouped_id",
            delete(handlers::delete),
        )
}

/// Who may send an album into `room_id`: a participant (else 404, never revealing the chat)
/// holding `message.send_media` and `message.send` (or `message.post` in a channel).
async fn authorize_send(
    state: &SharedState,
    room_id: Uuid,
    sender: &User,
) -> Result<(), AlbumError> {
    state.chat(room_id).await.ok_or(AlbumError::NotFound)?;
    if !state.is_chat_participant(room_id, sender.id).await? {
        return Err(AlbumError::NotFound);
    }
    let may_send = state
        .has_chat_permission(room_id, sender.id, "message.send")
        .await?
        || state
            .has_chat_permission(room_id, sender.id, "message.post")
            .await?;
    if !may_send
        || !state
            .has_chat_permission(room_id, sender.id, "message.send_media")
            .await?
    {
        return Err(AlbumError::Forbidden);
    }
    Ok(())
}

/// Validate, publish, insert atomically, then broadcast every item in album order.
pub async fn send_album(
    state: &SharedState,
    room_id: Uuid,
    sender: &User,
    request: SendAlbumRequest,
) -> Result<SendAlbumResponse, AlbumError> {
    let count = request.upload_ids.len();
    if !(MIN_ALBUM_ITEMS..=MAX_ALBUM_ITEMS).contains(&count) {
        return Err(AlbumError::Invalid("album_size"));
    }
    let mut unique = request.upload_ids.clone();
    unique.sort();
    unique.dedup();
    if unique.len() != count {
        return Err(AlbumError::Invalid("duplicate_upload"));
    }
    let caption = request.caption.trim();
    if caption.chars().count() > MAX_CAPTION_CHARS {
        return Err(AlbumError::Invalid("caption_too_long"));
    }
    authorize_send(state, room_id, sender).await?;
    let topic_id = state
        .resolve_post_topic(room_id, sender.id, request.topic_id)
        .await
        .map_err(AlbumError::from_topic)?;

    let mut sessions = Vec::with_capacity(count);
    for &upload_id in &request.upload_ids {
        let session = state
            .attachment_upload(upload_id)
            .await?
            .filter(|session| session.uploader_id == sender.id && session.room_id == room_id)
            .ok_or(AlbumError::NotFound)?;
        if session.status != "in_progress" {
            return Err(AlbumError::Conflict);
        }
        if session.received_bytes != session.declared_size_bytes {
            return Err(AlbumError::Invalid("upload_incomplete"));
        }
        if !is_album_media(&session.mime_type) {
            return Err(AlbumError::Invalid("not_media"));
        }
        sessions.push(session);
    }
    let reply_to = state.reply_preview(room_id, request.reply_to).await?;
    if request.reply_to.is_some() && reply_to.is_none() {
        return Err(AlbumError::Invalid("reply_not_found"));
    }

    let _permit = state
        .work_queue()
        .upload()
        .await
        .map_err(|_| AlbumError::Unavailable)?;
    let mut items = Vec::with_capacity(count);
    for session in &sessions {
        items.push(state.promote_album_upload(session).await?);
    }
    let display_name = state.resolve_display_name(room_id, sender).await;
    let (grouped_id, messages) = state
        .insert_album(
            NewAlbum {
                room_id,
                sender,
                sender_display_name: &display_name,
                caption,
                reply_to,
                is_sensitive: request.is_sensitive,
                silent: request.silent,
                topic_id,
            },
            &items,
        )
        .await?;
    for item in &items {
        state.upload_hashes().remove(item.upload_id).await;
    }
    state.invalidate_message_cache(room_id).await;
    for message in &messages {
        state
            .broadcast(room_id, stored_message_to_chat(message.clone()))
            .await;
    }
    Ok(SendAlbumResponse {
        grouped_id,
        messages,
    })
}

/// Delete (recall) the whole album: every item the caller sent in it, in one statement.
/// Deleting a single item stays the ordinary per-message recall.
pub async fn recall_album(
    state: &SharedState,
    room_id: Uuid,
    sender: &User,
    grouped_id: Uuid,
) -> Result<RecallAlbumResponse, AlbumError> {
    if !state.can_read_chat(room_id, sender.id).await? {
        return Err(AlbumError::NotFound);
    }
    let _permit = state
        .work_queue()
        .message()
        .await
        .map_err(|_| AlbumError::Unavailable)?;
    let recalled_at = Utc::now();
    let items = state
        .recall_album_items(room_id, sender.id, grouped_id, recalled_at)
        .await?;
    if items.is_empty() {
        return Err(AlbumError::NotFound);
    }
    for attachment_id in items.iter().filter_map(|(_, attachment)| *attachment) {
        if let Err(error) = state
            .recompute_attachment_orphan_status(attachment_id)
            .await
        {
            tracing::warn!("recompute attachment orphan status failed: {error:#}");
        }
    }
    state.invalidate_message_cache(room_id).await;
    for (message_id, _) in &items {
        state
            .broadcast(
                room_id,
                ChatMessage::MessageRecalled {
                    message_id: *message_id,
                    recalled_at,
                },
            )
            .await;
    }
    Ok(RecallAlbumResponse {
        recalled: items
            .into_iter()
            .map(|(message_id, _)| RecalledAlbumItem {
                message_id,
                recalled_at,
            })
            .collect(),
    })
}
