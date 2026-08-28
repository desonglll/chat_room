//! chat_room — Axum-based chat server with WebSocket and OpenAPI.
pub mod accounts;
pub mod admin;
pub mod ai;
pub mod ai_extractions;
pub mod ai_governance;
pub mod ai_handlers;
pub mod ai_suggestions;
pub mod ai_threads;
pub mod attachments;
pub mod audit;
pub mod backup;
mod cache;
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
pub mod rooms;
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
pub(crate) use attachments::content as attachment_content;
pub use attachments::{
    file_handlers, handlers as attachment_handlers, storage as attachment_storage,
    upload_handlers as attachment_upload_handlers, upload_sessions as attachment_upload_sessions,
};
use axum::{routing::get, Json, Router};
pub use messages::{
    actions as message_actions, forward_handlers, global_search as message_global_search,
    pins as message_pins, reactions as message_reactions, read_store, search as message_search,
    store as message_store,
};
pub use realtime::ws;
pub(crate) use realtime::{auth as ws_auth, inbound as ws_inbound};
pub use rooms::{
    access as room_access, handlers, membership_handlers, membership_mutations, participants,
    query_handlers as room_query_handlers,
};
use std::sync::Arc;
use utoipa::OpenApi;
#[derive(OpenApi)]
#[openapi(
    paths(
        handlers::create_room,
        room_query_handlers::list_rooms,
        room_query_handlers::discover_rooms,
        room_query_handlers::get_room,
        message_pins::list_pins,
        message_pins::pin_message,
        message_pins::unpin_message,
        handlers::update_room,
        handlers::delete_room,
        handlers::list_messages,
        message_search::search_messages,
        message_search::message_context,
        message_global_search::handlers::search_visible_messages,
        notifications::handlers::list,
        notifications::handlers::unread_count,
        notifications::handlers::mark_read,
        notifications::handlers::mark_all_read,
        push_notifications::handlers::public_config,
        push_notifications::handlers::save_subscription,
        push_notifications::handlers::delete_subscription,
        attachment_handlers::upload_attachment,
        attachment_handlers::download_attachment,
        attachment_upload_handlers::create_upload,
        attachment_upload_handlers::upload_chunk,
        attachment_upload_handlers::complete_upload,
        attachment_upload_handlers::list_uploads,
        attachment_upload_handlers::cancel_upload,
        file_handlers::list_room_files,
        registration::register,
        user_handlers::login,
        user_handlers::me,
        sessions::list,
        sessions::revoke,
        sessions::revoke_others,
        user_handlers::verify_password,
        user_handlers::get_user,
        user_handlers::update_me,
        user_handlers::delete_account,
        avatar_handlers::upload_avatar,
        avatar_handlers::download_avatar,
        user_handlers::logout,
        social::handlers::search_users,
        social::handlers::create_friend_request,
        social::handlers::list_friend_requests,
        social::handlers::update_friend_request,
        social::handlers::cancel_friend_request,
        social::handlers::list_friends,
        social::handlers::update_friend_remark,
        social::handlers::delete_friend,
        social::handlers::list_blocks,
        social::handlers::block_user,
        social::handlers::unblock_user,
        direct_conversations::handlers::start_direct_chat,
        conversations::handlers::list_conversations,
        conversations::handlers::update_conversation_alias,
        conversations::handlers::get_conversation_preferences,
        conversations::handlers::update_conversation_preferences,
        forward_handlers::forward_messages,
        favorites::handlers::list_favorites,
        favorites::handlers::create_favorite,
        favorites::handlers::upload_favorite_attachment,
        favorites::handlers::update_favorite,
        favorites::handlers::favorite_messages,
        favorites::handlers::delete_favorite,
        favorites::handlers::forward_favorite,
        favorites::handlers::list_collaborators,
        favorites::handlers::add_collaborator,
        favorites::handlers::remove_collaborator,
        ai_suggestions::suggest,
        ai_suggestions::suggest_events,
        ai::model_handlers::list_models,
        ai_threads::handlers::list_threads,
        ai_threads::handlers::create_thread,
        ai_threads::handlers::update_thread,
        ai_threads::handlers::delete_thread,
        ai_threads::handlers::list_messages,
        ai_threads::runs::create_run,
        ai_threads::catch_up::create_catch_up,
        ai_threads::runs::get_run,
        ai_threads::events::stream_run_events,
        ai_extractions::handlers::create,
        ai_extractions::handlers::get,
        ai_extractions::handlers::update_candidate,
        ai_governance::handlers::room_policy,
        ai_governance::handlers::update_room_policy,
        ai_governance::handlers::admin_settings,
        ai_governance::handlers::update_admin_settings,
        ai_governance::handlers::admin_usage,
        tasks::handlers::list,
        tasks::handlers::create,
        tasks::handlers::update,
        tasks::handlers::delete,
        admin_metrics::overview,
        admin_metrics::purge,
        admin_backups::export,
        admin_backups::restore,
        admin_backups::execute_restore,
        admin_backups::get_status,
        admin_backups::run_now,
        admin_services::probe_vector_search,
        admin_ai_models::list,
        admin_ai_models::create,
        admin_ai_models::update,
        admin_ai_models::delete,
        admin_system_lock::update,
        admin_system_lock::room_status,
        admin_system_lock::update_room,
        admin_system_admins::handlers::list,
        admin_system_admins::handlers::grant,
        admin_system_admins::handlers::revoke,
        admin_system_admins::handlers::create_invite,
        audit::handlers::list_system,
        audit::handlers::list_room,
    ),
    components(schemas(
        ai::AiSuggestions,
        ai::AiConversationTurn,
        ai::AiModelChoice,
        ai::AiModelOptionView,
        ai::SaveAiModelOption,
        ai_threads::AiThread,
        ai_threads::AiThreadMessage,
        ai_threads::CreateAiThreadRequest,
        ai_threads::UpdateAiThreadRequest,
        ai_threads::CreateAiRunRequest,
        ai_threads::AiRun,
        admin_services::ServiceStatus,
        admin_services::VectorIndexStatus,
        admin_services::ServiceOverview,
        admin_services::VectorProbeRequest,
        admin_services::VectorProbeMatch,
        admin_services::VectorProbeResult,
        models::Room,
        models::CreateRoomRequest,
        models::UpdateRoomRequest,
        models::StoredMessage,
        models::Attachment,
        models::ChatFileItem,
        models::ChatFilePage,
        models::ReplyPreview,
        models::ForwardedFrom,
        models::MessageReaction,
        models::ForwardMessagesRequest,
        models::ForwardResult,
        message_global_search::models::SearchContentType,
        message_global_search::models::GlobalMessageSearchResult,
        message_global_search::models::GlobalMessageSearchPage,
        notifications::NotificationActor,
        notifications::NotificationKind,
        notifications::NotificationView,
        notifications::NotificationPage,
        notifications::UnreadCount,
        push_notifications::handlers::WebPushPublicConfig,
        push_notifications::SavePushSubscriptionRequest,
        push_notifications::DeletePushSubscriptionRequest,
        push_notifications::PushSubscriptionKeys,
        push_notifications::PushSubscriptionView,
        favorites::models::FavoriteItem,
        favorites::models::CreateFavoriteRequest,
        favorites::models::UpdateFavoriteRequest,
        favorites::models::FavoriteCollaborator,
        favorites::models::AddFavoriteCollaboratorRequest,
        favorites::models::FavoriteMessagesRequest,
        favorites::models::ForwardFavoriteRequest,
        favorites::models::FavoriteForwardResult,
        message_pins::RoomPin,
        attachment_upload_handlers::CreateUploadRequest,
        attachment_upload_handlers::CreateUploadResponse,
        attachment_upload_handlers::ChunkResponse,
        attachment_upload_handlers::CompleteUploadRequest,
        attachment_storage::DirectUploadTarget,
        attachment_upload_sessions::AttachmentUploadSession,
        models::User,
        models::UserSummary,
        models::AuthRequest,
        registration::RegisterRequest,
        ai_threads::CreateCatchUpRunRequest,
        ai_extractions::AiExtractionSource,
        ai_extractions::AiExtractionCandidate,
        ai_extractions::AiExtractionRun,
        ai_extractions::CreateAiExtractionRequest,
        ai_extractions::UpdateAiExtractionCandidateRequest,
        ai_governance::RoomAiPolicy,
        ai_governance::UpdateRoomAiPolicy,
        ai_governance::AiGovernanceSettings,
        ai_governance::UpdateAiGovernanceSettings,
        ai_governance::AiUsageReport,
        tasks::RoomTask,
        tasks::RoomTaskSource,
        tasks::CreateRoomTaskRequest,
        tasks::UpdateRoomTaskRequest,
        models::AuthSession,
        sessions::DeviceSession,
        models::UpdateProfileRequest,
        models::ChangePasswordRequest,
        models::DeleteAccountRequest,
        models::VerifyPasswordRequest,
        models::JoinRoomRequest,
        models::InviteMemberRequest,
        models::UpdateMembershipRequest,
        models::UpdateNicknameRequest,
        models::RoomMembership,
        social::models::SocialUser,
        social::models::FriendRequestView,
        social::models::FriendRequestPayload,
        social::models::FriendRequestAction,
        social::models::UpdateFriendRemarkRequest,
        conversations::models::ConversationSummary,
        conversations::models::ConversationPreferences,
        conversations::models::MessagePreview,
        conversations::models::NotificationLevel,
        conversations::models::UpdateConversationAliasRequest,
        conversations::models::UpdateConversationPreferencesRequest,
        admin_metrics::AdminOverview,
        admin_metrics::PurgeResult,
        admin_backups::RestoreBackupResult,
        admin_backups::RestoreValidationResult,
        admin_backups::BackupApiError,
        backup::BackupRun,
        backup::BackupStatus,
        admin_system_lock::SystemLockStatus,
        admin_system_lock::UpdateSystemLockRequest,
        admin_system_lock::RoomLockStatus,
        admin_system_admins::SystemAdminView,
        admin_system_admins::CreateRegistrationInviteRequest,
        admin_system_admins::RegistrationInviteSecret,
        audit::AuditEvent,
        audit::AuditEventPage,
    ))
)]
pub struct ApiDoc;
/// Serve the OpenAPI JSON spec at /api-docs/openapi.json.
async fn openapi_json() -> Json<utoipa::openapi::OpenApi> {
    Json(ApiDoc::openapi())
}
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
        .route("/api-docs/openapi.json", get(openapi_json));
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
