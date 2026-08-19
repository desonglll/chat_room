//! chat_room — Axum-based chat server with WebSocket and OpenAPI.

pub mod accounts;
pub mod admin;
pub mod ai;
pub mod ai_handlers;
pub mod attachments;
pub mod config;
pub mod messages;
pub mod models;
pub mod realtime;
pub mod rooms;
pub mod state;
mod state_runtime;
pub mod storage;
pub mod web;

pub use accounts::{account_ws, user_handlers, users};
pub use admin::metrics as admin_metrics;
pub(crate) use attachments::content as attachment_content;
pub use attachments::{
    file_handlers, handlers as attachment_handlers, storage as attachment_storage,
    upload_handlers as attachment_upload_handlers, upload_sessions as attachment_upload_sessions,
};
pub use messages::{
    actions as message_actions, forward_handlers, read_store, store as message_store,
};
pub use realtime::ws;
pub(crate) use realtime::{auth as ws_auth, inbound as ws_inbound};
pub use rooms::{
    access as room_access, handlers, membership_handlers, membership_mutations, participants,
};

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
        attachment_upload_handlers::create_upload,
        attachment_upload_handlers::upload_chunk,
        attachment_upload_handlers::complete_upload,
        attachment_upload_handlers::list_uploads,
        attachment_upload_handlers::cancel_upload,
        file_handlers::list_room_files,
        user_handlers::register,
        user_handlers::login,
        user_handlers::me,
        user_handlers::verify_password,
        user_handlers::get_user,
        user_handlers::update_me,
        user_handlers::logout,
        forward_handlers::forward_messages,
        ai_handlers::suggest,
        admin_metrics::overview,
        admin_metrics::purge,
    ),
    components(schemas(
        ai::AiSuggestions,
        models::Room,
        models::CreateRoomRequest,
        models::UpdateRoomRequest,
        models::StoredMessage,
        models::Attachment,
        models::ChatFileItem,
        models::ChatFilePage,
        models::ReplyPreview,
        models::ForwardedFrom,
        models::ForwardMessagesRequest,
        models::ForwardResult,
        attachment_upload_handlers::CreateUploadRequest,
        attachment_upload_handlers::CreateUploadResponse,
        attachment_upload_handlers::ChunkResponse,
        attachment_upload_handlers::CompleteUploadRequest,
        attachment_upload_sessions::AttachmentUploadSession,
        models::User,
        models::AuthRequest,
        models::AuthSession,
        models::UpdateProfileRequest,
        models::ChangePasswordRequest,
        models::DeleteAccountRequest,
        models::VerifyPasswordRequest,
        models::JoinRoomRequest,
        models::InviteMemberRequest,
        models::UpdateMembershipRequest,
        models::UpdateNicknameRequest,
        models::RoomMembership,
        admin_metrics::AdminOverview,
        admin_metrics::PurgeResult,
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
    let chunk_body_limit = state.chunk_body_limit_bytes();
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
            "/api/rooms/:id/ai/suggest",
            axum::routing::post(ai_handlers::suggest),
        )
        .route(
            "/api/rooms/:id/members/me",
            axum::routing::delete(membership_handlers::leave_room)
                .patch(membership_handlers::update_own_nickname),
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
            "/api/rooms/:id/attachments/uploads",
            axum::routing::post(attachment_upload_handlers::create_upload)
                .get(attachment_upload_handlers::list_uploads),
        )
        .route(
            "/api/attachments/uploads/:id/chunks",
            axum::routing::put(attachment_upload_handlers::upload_chunk)
                .layer(axum::extract::DefaultBodyLimit::max(chunk_body_limit)),
        )
        .route(
            "/api/attachments/uploads/:id/complete",
            axum::routing::post(attachment_upload_handlers::complete_upload),
        )
        .route(
            "/api/attachments/uploads/:id",
            axum::routing::delete(attachment_upload_handlers::cancel_upload),
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
            "/api/users/me/verify-password",
            axum::routing::post(user_handlers::verify_password),
        )
        .route("/api/users/:id", get(user_handlers::get_user))
        .route(
            "/api/messages/forward",
            axum::routing::post(forward_handlers::forward_messages),
        )
        .route(
            "/api/users/logout",
            axum::routing::post(user_handlers::logout),
        )
        .route("/api/admin/overview", get(admin_metrics::overview))
        .route(
            "/api/admin/maintenance/purge",
            axum::routing::post(admin_metrics::purge),
        )
        .route("/ws/account", get(account_ws::account_ws_handler))
        .route("/ws/:room_id", get(ws::ws_handler))
        .route("/api-docs/openapi.json", get(openapi_json));

    if web_enabled {
        app = app
            .route("/", get(web::index))
            .route("/favicon.svg", get(web::favicon))
            .route("/assets/*path", get(web::bundled_asset))
            .route("/icons/icon-sprite.svg", get(web::icon_sprite))
            .route("/brand/echo-gate.svg", get(web::echo_gate))
            .route("/emoji-data-zh.json", get(web::emoji_data_zh))
            // The Vue client uses history-mode client-side routing (/rooms/:id, /profile,
            // /settings) — any path not matched above is a client route, not a 404.
            .fallback(get(web::index));
    }

    app.layer(axum::middleware::from_fn_with_state(
        state.clone(),
        admin_metrics::track_request,
    ))
    .layer(CorsLayer::permissive())
    .layer(TraceLayer::new_for_http())
    .with_state(state)
}
