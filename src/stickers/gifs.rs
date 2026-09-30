//! GIFs (TG-305): each account's saved GIFs, the GIFs recently sent into its chats, and GIF
//! messages.
//!
//! A GIF message is an attachment message with `messages.media_kind = 'gif'`, so chat
//! authorization, realtime delivery, history, recall and the orphan sweep are the existing
//! attachment paths. The stored format is whatever was uploaded — a real GIF, or a short
//! silent H.264 MP4 / WebM "animation" — because the server has no transcoder (see
//! `docs/devlog/TG-305.md` Decisions); clients play the video formats muted and looping and
//! fall back to `<img>` for a real GIF.
//!
//! The contract is frozen in `docs/devlog/TG-305.md`.

pub mod animation;
pub mod files;
pub mod handlers;
pub mod library;
pub mod models;
pub mod send;

use std::sync::Arc;

use axum::{
    extract::DefaultBodyLimit,
    routing::{delete, get, post},
    Router,
};

use crate::state::AppState;
use animation::MAX_ANIMATION_BYTES;

/// Every GIF route. Merged into `crate::stickers::routes`.
pub(crate) fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/api/gifs/saved",
            get(handlers::list_saved).post(handlers::save),
        )
        .route("/api/gifs/saved/:id", delete(handlers::remove))
        .route("/api/gifs/saved/:id/file", get(files::download_saved_gif))
        .route("/api/gifs/recent", get(handlers::recent))
        .route("/api/chats/:id/gif-messages", post(handlers::send))
        .route(
            "/api/chats/:id/gif-messages/upload",
            post(handlers::upload).layer(DefaultBodyLimit::max(MAX_ANIMATION_BYTES)),
        )
}
