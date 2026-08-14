//! chat_room — Axum-based chat server with WebSocket and OpenAPI.

pub mod handlers;
pub mod models;
pub mod state;
pub mod storage;
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
    paths(handlers::create_room, handlers::list_rooms, handlers::get_room,),
    components(schemas(models::Room, models::CreateRoomRequest,))
)]
pub struct ApiDoc;

/// Serve the OpenAPI JSON spec at /api-docs/openapi.json.
async fn openapi_json() -> Json<utoipa::openapi::OpenApi> {
    Json(ApiDoc::openapi())
}

// ── Router factory ──────────────────────────────────────────────────────────

/// Build the complete axum Router with all routes.
pub fn build_app(state: Arc<AppState>) -> Router {
    Router::new()
        .route(
            "/api/rooms",
            get(handlers::list_rooms).post(handlers::create_room),
        )
        .route("/api/rooms/:id", get(handlers::get_room))
        .route("/ws/:room_id", get(ws::ws_handler))
        .route("/api-docs/openapi.json", get(openapi_json))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
