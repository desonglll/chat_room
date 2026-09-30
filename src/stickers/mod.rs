//! Sticker sets, stickers, each account's sticker library, and sticker messages (TG-302).
//!
//! The contract is frozen in `docs/devlog/TG-302.md`. Sticker files share the attachment
//! object store; a sticker sent into a chat becomes an ordinary attachment row plus
//! `messages.media_kind = 'sticker'`, so chat authorization, realtime delivery and history
//! are the existing attachment paths.

pub(crate) mod catalogue;
pub mod custom_emoji;
pub mod errors;
pub mod files;
pub mod gifs;
pub mod handlers;
pub mod library;
pub(crate) mod message_view;
pub mod models;
pub mod recents;
pub mod send;
pub mod sets;
pub mod validation;

use std::sync::Arc;

use axum::{
    extract::DefaultBodyLimit,
    routing::{get, post, put},
    Router,
};

use crate::attachment_handlers::MULTIPART_OVERHEAD_BYTES;
use crate::state::AppState;
use handlers::{library as library_api, messages as message_api, sets as set_api};
use models::StickerFormat;

/// Every sticker route. Mounted once from `crate::routes::api_routes`.
pub(crate) fn routes() -> Router<Arc<AppState>> {
    let upload_limit = StickerFormat::Webp.max_bytes() + MULTIPART_OVERHEAD_BYTES;
    Router::new()
        .route("/api/sticker-sets", post(set_api::create_set))
        .route("/api/sticker-sets/:short_name", get(set_api::get_set))
        .route(
            "/api/sticker-sets/:short_name/stickers",
            post(set_api::add_sticker).layer(DefaultBodyLimit::max(upload_limit)),
        )
        .route(
            "/api/sticker-sets/:short_name/stickers/:sticker_id",
            axum::routing::delete(set_api::remove_sticker),
        )
        .route(
            "/api/stickers/installed",
            get(library_api::list_installed).put(library_api::reorder),
        )
        .route(
            "/api/stickers/installed/:set_id",
            put(library_api::install)
                .patch(library_api::archive)
                .delete(library_api::uninstall),
        )
        .route("/api/stickers/recent", get(library_api::recent))
        .route(
            "/api/stickers/recent/:sticker_id",
            axum::routing::delete(library_api::remove_recent),
        )
        .route("/api/stickers/favorites", get(library_api::favorites))
        .route(
            "/api/stickers/favorites/:sticker_id",
            put(library_api::add_favorite).delete(library_api::remove_favorite),
        )
        .route("/api/stickers/search", get(library_api::search))
        .route(
            "/api/stickers/:sticker_id/file",
            get(files::download_sticker_file),
        )
        .route(
            "/api/chats/:id/sticker-messages",
            post(message_api::send_sticker),
        )
        .merge(custom_emoji::routes())
        .merge(gifs::routes())
}
