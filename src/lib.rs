//! chat_room — Axum-based chat server with WebSocket and OpenAPI.

mod account_events;
pub mod account_ws;
pub mod attachment_handlers;
pub mod attachment_storage;
pub mod config;
pub mod file_handlers;
pub mod handlers;
pub mod membership_handlers;
pub mod membership_mutations;
pub mod message_actions;
pub mod message_store;
pub mod models;
pub mod participants;
pub mod read_store;
pub mod room_access;
pub mod state;
pub mod storage;
pub mod user_handlers;
pub mod users;
pub mod web;
pub mod ws;
mod ws_auth;
mod ws_inbound;

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
        file_handlers::list_room_files,
        user_handlers::register,
        user_handlers::login,
        user_handlers::me,
        user_handlers::update_me,
        user_handlers::logout,
    ),
    components(schemas(
        models::Room,
        models::CreateRoomRequest,
        models::UpdateRoomRequest,
        models::StoredMessage,
        models::Attachment,
        models::ChatFileItem,
        models::ChatFilePage,
        models::ReplyPreview,
        models::User,
        models::AuthRequest,
        models::AuthSession,
        models::UpdateProfileRequest,
        models::ChangePasswordRequest,
        models::DeleteAccountRequest,
        models::JoinRoomRequest,
        models::InviteMemberRequest,
        models::UpdateMembershipRequest,
        models::RoomMembership,
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
    let multipart_body_limit = state
        .max_upload_bytes()
        .saturating_add(attachment_handlers::MULTIPART_OVERHEAD_BYTES);
    let mut app = Router::new()
        .route("/api/config", get(config::public_config))
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
        .route("/api/rooms/:id/files", get(file_handlers::list_room_files))
        .route(
            "/api/rooms/:id/members/me",
            axum::routing::delete(membership_handlers::leave_room),
        )
        .route(
            "/api/rooms/:id/join-requests",
            axum::routing::post(membership_handlers::request_join),
        )
        .route(
            "/api/rooms/:id/members",
            get(membership_handlers::list_members),
        )
        .route(
            "/api/rooms/:id/invitations",
            axum::routing::post(membership_handlers::invite_member),
        )
        .route(
            "/api/rooms/:id/members/:user_id",
            axum::routing::patch(membership_handlers::update_member),
        )
        .route(
            "/api/rooms/:id/attachments",
            axum::routing::post(attachment_handlers::upload_attachment)
                .layer(axum::extract::DefaultBodyLimit::max(multipart_body_limit)),
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
        .route(
            "/api/users/me",
            get(user_handlers::me)
                .patch(user_handlers::update_me)
                .delete(user_handlers::delete_account),
        )
        .route(
            "/api/users/me/password",
            axum::routing::put(user_handlers::change_password),
        )
        .route(
            "/api/users/logout",
            axum::routing::post(user_handlers::logout),
        )
        .route("/ws/account", get(account_ws::account_ws_handler))
        .route("/ws/:room_id", get(ws::ws_handler))
        .route("/api-docs/openapi.json", get(openapi_json));

    if web_enabled {
        app = app
            .route("/", get(web::index))
            .route("/favicon.svg", get(web::favicon))
            .route("/assets/app.css", get(web::stylesheet))
            .route("/assets/app.js", get(web::app_script))
            .route("/assets/jszip.min.js", get(web::jszip_script));
    }

    app.layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
