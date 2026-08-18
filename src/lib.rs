//! chat_room — Axum-based chat server with WebSocket and OpenAPI.

pub mod attachment_handlers;
pub mod handlers;
pub mod message_store;
pub mod models;
pub mod state;
pub mod storage;
pub mod user_handlers;
pub mod users;
pub mod web;
pub mod ws;

use axum::{routing::get, Json, Router};
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use utoipa::OpenApi;

use crate::state::AppState;

// ── OpenAPI document ────────────────────────────────────────────────────────

#[derive(OpenApi)]
#[openapi(
    paths(
        handlers::create_room,
        handlers::list_rooms,
        handlers::get_room,
        handlers::update_room,
        handlers::delete_room,
        handlers::list_messages,
        attachment_handlers::upload_attachment,
        attachment_handlers::download_attachment,
        user_handlers::register,
        user_handlers::login,
        user_handlers::me,
        user_handlers::logout,
    ),
    components(schemas(
        models::Room,
        models::CreateRoomRequest,
        models::UpdateRoomRequest,
        models::StoredMessage,
        models::Attachment,
        models::User,
        models::AuthRequest,
        models::AuthSession,
    ))
)]
pub struct ApiDoc;

/// Serve the OpenAPI JSON spec at /api-docs/openapi.json.
async fn openapi_json() -> Json<utoipa::openapi::OpenApi> {
    Json(ApiDoc::openapi())
}

// ── Router factory ──────────────────────────────────────────────────────────

/// Build the API-only axum router.
pub fn build_app(state: Arc<AppState>) -> Router {
    build_app_with_web(state, false)
}

/// Build the axum router and optionally serve the embedded browser client.
pub fn build_app_with_web(state: Arc<AppState>, web_enabled: bool) -> Router {
    let mut app = Router::new()
        .route(
            "/api/rooms",
            get(handlers::list_rooms).post(handlers::create_room),
        )
        .route(
            "/api/rooms/:id",
            get(handlers::get_room)
                .patch(handlers::update_room)
                .delete(handlers::delete_room),
        )
        .route("/api/rooms/:id/messages", get(handlers::list_messages))
        .route(
            "/api/rooms/:id/attachments",
            axum::routing::post(attachment_handlers::upload_attachment).layer(
                axum::extract::DefaultBodyLimit::max(attachment_handlers::MULTIPART_BODY_LIMIT),
            ),
        )
        .route(
            "/api/attachments/:id",
            get(attachment_handlers::download_attachment),
        )
        .route(
            "/api/users/register",
            axum::routing::post(user_handlers::register),
        )
        .route(
            "/api/users/login",
            axum::routing::post(user_handlers::login),
        )
        .route("/api/users/me", get(user_handlers::me))
        .route(
            "/api/users/logout",
            axum::routing::post(user_handlers::logout),
        )
        .route("/ws/:room_id", get(ws::ws_handler))
        .route("/api-docs/openapi.json", get(openapi_json));

    if web_enabled {
        app = app
            .route("/", get(web::index))
            .route("/favicon.svg", get(web::favicon))
            .route("/assets/app.css", get(web::stylesheet))
            .route("/assets/app.js", get(web::app_script));
    }

    app.layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
