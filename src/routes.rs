//! HTTP and WebSocket route registration.

use std::sync::Arc;

use axum::{routing::get, Router};

use crate::{
    account_ws, admin, admin_ai_models, admin_backups, admin_metrics, admin_services,
    admin_system_lock, ai, ai_handlers, ai_threads, attachment_handlers,
    attachment_upload_handlers, avatar_handlers, config, conversations, direct_conversations,
    favorites, file_handlers, forward_handlers, handlers, membership_handlers,
    message_global_search, message_pins, message_search, notifications, room_query_handlers,
    social, state::AppState, user_handlers, ws,
};

pub(crate) fn api_routes(
    multipart_body_limit: usize,
    chunk_body_limit: usize,
) -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/config", get(config::public_config))
        .route(
            "/api/rooms",
            get(room_query_handlers::list_rooms).post(handlers::create_room),
        )
        .route(
            "/api/rooms/discover",
            get(room_query_handlers::discover_rooms),
        )
        .route(
            "/api/rooms/:id",
            get(room_query_handlers::get_room)
                .patch(handlers::update_room)
                .delete(handlers::delete_room),
        )
        .route("/api/rooms/:id/messages", get(handlers::list_messages))
        .route("/api/rooms/:id/pins", get(message_pins::list_pins))
        .route(
            "/api/rooms/:id/pins/:message_id",
            axum::routing::post(message_pins::pin_message).delete(message_pins::unpin_message),
        )
        .route(
            "/api/rooms/:id/messages/search",
            get(message_search::search_messages),
        )
        .route(
            "/api/messages/search",
            get(message_global_search::handlers::search_visible_messages),
        )
        .route(
            "/api/rooms/:id/messages/:message_id/context",
            get(message_search::message_context),
        )
        .route("/api/rooms/:id/files", get(file_handlers::list_room_files))
        .route(
            "/api/rooms/:id/ai/suggest",
            axum::routing::post(ai_handlers::suggest),
        )
        .route(
            "/api/rooms/:id/ai/suggest/events",
            axum::routing::post(ai_handlers::suggest_events),
        )
        .route(
            "/api/ai/threads",
            get(ai_threads::handlers::list_threads).post(ai_threads::handlers::create_thread),
        )
        .route("/api/ai/models", get(ai::model_handlers::list_models))
        .route(
            "/api/ai/threads/:id",
            axum::routing::patch(ai_threads::handlers::update_thread)
                .delete(ai_threads::handlers::delete_thread),
        )
        .route(
            "/api/ai/threads/:id/messages",
            get(ai_threads::handlers::list_messages),
        )
        .route(
            "/api/ai/threads/:id/runs",
            axum::routing::post(ai_threads::runs::create_run),
        )
        .route("/api/ai/runs/:id", get(ai_threads::runs::get_run))
        .route(
            "/api/ai/runs/:id/events",
            get(ai_threads::events::stream_run_events),
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
            "/api/users/me/avatar",
            axum::routing::post(avatar_handlers::upload_avatar).layer(
                axum::extract::DefaultBodyLimit::max(
                    avatar_handlers::MAX_AVATAR_BYTES + avatar_handlers::MULTIPART_OVERHEAD_BYTES,
                ),
            ),
        )
        .route(
            "/api/users/me/verify-password",
            axum::routing::post(user_handlers::verify_password),
        )
        .route("/api/users/search", get(social::handlers::search_users))
        .route(
            "/api/users/:id/avatar",
            get(avatar_handlers::download_avatar),
        )
        .route("/api/users/:id", get(user_handlers::get_user))
        .route(
            "/api/friend-requests",
            get(social::handlers::list_friend_requests)
                .post(social::handlers::create_friend_request),
        )
        .route(
            "/api/friend-requests/:user_id",
            axum::routing::patch(social::handlers::update_friend_request)
                .delete(social::handlers::cancel_friend_request),
        )
        .route("/api/friends", get(social::handlers::list_friends))
        .route(
            "/api/friends/:user_id/remark",
            axum::routing::put(social::handlers::update_friend_remark),
        )
        .route(
            "/api/friends/:user_id",
            axum::routing::delete(social::handlers::delete_friend),
        )
        .route("/api/blocks", get(social::handlers::list_blocks))
        .route(
            "/api/blocks/:user_id",
            axum::routing::put(social::handlers::block_user).delete(social::handlers::unblock_user),
        )
        .route(
            "/api/direct-chats",
            axum::routing::post(direct_conversations::handlers::start_direct_chat),
        )
        .route(
            "/api/conversations",
            get(conversations::handlers::list_conversations),
        )
        .route(
            "/api/conversations/:room_id/alias",
            axum::routing::put(conversations::handlers::update_conversation_alias),
        )
        .route(
            "/api/conversations/:room_id/preferences",
            get(conversations::handlers::get_conversation_preferences)
                .patch(conversations::handlers::update_conversation_preferences),
        )
        .route("/api/notifications", get(notifications::handlers::list))
        .route(
            "/api/notifications/unread-count",
            get(notifications::handlers::unread_count),
        )
        .route(
            "/api/notifications/read-all",
            axum::routing::post(notifications::handlers::mark_all_read),
        )
        .route(
            "/api/notifications/:id/read",
            axum::routing::post(notifications::handlers::mark_read),
        )
        .route(
            "/api/messages/forward",
            axum::routing::post(forward_handlers::forward_messages),
        )
        .route(
            "/api/favorites",
            get(favorites::handlers::list_favorites).post(favorites::handlers::create_favorite),
        )
        .route(
            "/api/favorites/messages",
            axum::routing::post(favorites::handlers::favorite_messages),
        )
        .route(
            "/api/favorites/attachments",
            axum::routing::post(favorites::handlers::upload_favorite_attachment)
                .layer(axum::extract::DefaultBodyLimit::max(multipart_body_limit)),
        )
        .route(
            "/api/favorites/:id",
            axum::routing::put(favorites::handlers::update_favorite)
                .delete(favorites::handlers::delete_favorite),
        )
        .route(
            "/api/favorites/:id/collaborators",
            get(favorites::handlers::list_collaborators)
                .post(favorites::handlers::add_collaborator),
        )
        .route(
            "/api/favorites/:id/collaborators/:user_id",
            axum::routing::delete(favorites::handlers::remove_collaborator),
        )
        .route(
            "/api/favorites/:id/forward",
            axum::routing::post(favorites::handlers::forward_favorite),
        )
        .route(
            "/api/users/logout",
            axum::routing::post(user_handlers::logout),
        )
        .route("/api/admin/overview", get(admin_metrics::overview))
        .route(
            "/api/admin/vector/probe",
            axum::routing::post(admin_services::probe_vector_search),
        )
        .route(
            "/api/admin/ai-models",
            get(admin_ai_models::list).post(admin_ai_models::create),
        )
        .route(
            "/api/admin/ai-models/:id",
            axum::routing::put(admin_ai_models::update).delete(admin_ai_models::delete),
        )
        .route(
            "/api/admin/maintenance/purge",
            axum::routing::post(admin_metrics::purge),
        )
        .merge(admin::indexes::routes())
        .merge(admin_backups::routes())
        .route(
            "/api/admin/chat-lock",
            axum::routing::put(admin_system_lock::update),
        )
        .route(
            "/api/admin/room-locks/:room_id",
            get(admin_system_lock::room_status).put(admin_system_lock::update_room),
        )
        .route("/ws/account", get(account_ws::account_ws_handler))
        .route("/ws/:room_id", get(ws::ws_handler))
}
