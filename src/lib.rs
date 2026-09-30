//! chat_room — Axum-based chat server with WebSocket and OpenAPI.
pub mod accounts;
pub mod admin;
pub mod ai;
pub mod ai_extractions;
pub mod ai_governance;
pub mod ai_handlers;
pub mod ai_suggestions;
pub mod ai_threads;
mod api_doc;
pub mod attachments;
pub mod audit;
pub mod backup;
mod cache;
pub mod chats;
pub mod config;
pub mod conversations;
pub mod direct_conversations;
pub mod favorites;
pub mod knowledge;
pub mod messages;
pub mod models;
pub mod notifications;
pub mod observability;
pub mod push_notifications;
pub mod realtime;
mod routes;
mod security;
pub mod social;
pub mod state;
mod state_backup;
mod state_build;
mod state_runtime;
pub mod storage;
pub mod tasks;
pub mod web;
mod work_queue;
use crate::state::AppState;
pub use accounts::{account_ws, avatar_handlers, registration, sessions, user_handlers, users};
pub use admin::{
    ai_models as admin_ai_models, backups as admin_backups, metrics as admin_metrics,
    services as admin_services, system_admins as admin_system_admins,
    system_lock as admin_system_lock,
};
pub use api_doc::ApiDoc;
pub(crate) use attachments::content as attachment_content;
pub use attachments::{
    file_handlers, handlers as attachment_handlers, storage as attachment_storage,
    upload_handlers as attachment_upload_handlers, upload_sessions as attachment_upload_sessions,
};
use axum::{routing::get, Router};
pub use chats::{
    access as chat_access, handlers, membership_handlers, membership_mutations, participants,
    query_handlers as chat_query_handlers,
};
pub use messages::{
    actions as message_actions, forward_handlers, global_search as message_global_search,
    pins as message_pins, reactions as message_reactions, read_store, search as message_search,
    store as message_store,
};
pub use realtime::ws;
pub(crate) use realtime::{auth as ws_auth, inbound as ws_inbound};
use std::sync::Arc;

/// Build the API-only axum router.
pub fn build_app(state: Arc<AppState>) -> Router {
    build_app_with_web(state, false)
}
/// Build the axum router and optionally serve the embedded browser client.
pub fn build_app_with_web(state: Arc<AppState>, web_enabled: bool) -> Router {
    ai_threads::runs::ensure_dispatcher(state.clone());
    ai_extractions::ensure_dispatcher(state.clone());
    knowledge::ensure_worker(state.clone());
    push_notifications::delivery::ensure_dispatcher(state.clone());
    backup::ensure_scheduler(state.clone());
    let multipart_body_limit = state
        .max_upload_bytes()
        .saturating_add(attachment_handlers::MULTIPART_OVERHEAD_BYTES);
    let chunk_body_limit = state.chunk_body_limit_bytes();
    let cors = security::cors_layer(&state.config.security);
    let mut app = routes::api_routes(multipart_body_limit, chunk_body_limit)
        .route("/api-docs/openapi.json", get(chats::compat::openapi_json));
    if web_enabled {
        app = app
            .route("/", get(web::index))
            .route("/favicon.svg", get(web::favicon))
            .route("/manifest.webmanifest", get(web::manifest))
            .route("/sw.js", get(web::service_worker))
            .route("/pwa-192.png", get(web::pwa_icon_192))
            .route("/pwa-512.png", get(web::pwa_icon_512))
            .route("/assets/*path", get(web::bundled_asset))
            .route("/icons/icon-sprite.svg", get(web::icon_sprite))
            .route("/brand/echo-gate.svg", get(web::echo_gate))
            .route("/emoji-data-zh.json", get(web::emoji_data_zh))
            .route("/theme-bootstrap.js", get(web::theme_bootstrap))
            // The Vue client uses history-mode client-side routing (/rooms/:id, /profile,
            // /settings) — any path not matched above is a client route, not a 404.
            .fallback(get(web::index));
    }
    app.layer(axum::middleware::from_fn_with_state(
        state.clone(),
        admin_metrics::track_request,
    ))
    .layer(axum::middleware::from_fn_with_state(
        state.clone(),
        admin_backups::reject_during_restore,
    ))
    .layer(cors)
    .layer(axum::middleware::from_fn(security::security_headers))
    .layer(axum::middleware::from_fn(observability::request_context))
    .with_state(state)
}
