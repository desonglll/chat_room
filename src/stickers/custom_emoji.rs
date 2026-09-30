//! Custom emoji (TG-304): message entities, the custom emoji catalogue, and emoji status.
//!
//! A custom emoji is a sticker of a `custom_emoji` set (TG-302 tables and upload validation).
//! Messages reference one through a `custom_emoji` **entity**: a UTF-16 range of the message
//! text whose characters are the fallback emoji, plus the sticker id. The text stays the
//! source of truth — a client that ignores entities, or a viewer whose client cannot load the
//! file, still reads the fallback emoji. The contract is frozen in `docs/devlog/TG-304.md`.

pub mod catalogue;
pub mod entities;
pub mod entity_store;
pub mod handlers;
pub mod models;
pub mod status;

use std::sync::Arc;

use axum::{
    routing::{get, put},
    Router,
};

use crate::state::AppState;

pub use entities::MessageEntity;

/// Every custom emoji route. Merged into the sticker router (`super::routes`).
pub(crate) fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/custom-emoji", get(handlers::resolve))
        .route("/api/custom-emoji/installed", get(handlers::installed))
        .route(
            "/api/users/me/emoji-status",
            put(handlers::set_status).delete(handlers::clear_status),
        )
        .route("/api/users/emoji-statuses", get(handlers::statuses))
}
