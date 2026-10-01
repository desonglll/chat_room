//! The chat-scoped route tree, mounted twice.
//!
//! `docs/tg/architecture.md` §5.1: `/api/chats/*` is the contract and `/api/rooms/*` is a
//! deprecated alias that forwards to the same handlers and is removed in M6.

use std::sync::Arc;

use axum::{routing::get, Router};

use super::{
    admin_handlers, drafts, handlers, lifecycle_handlers, membership_handlers, message_history,
    query_handlers as chat_query_handlers, roster_handlers, slow_mode,
    topics::handlers as topic_handlers, topics::viewer_handlers as topic_viewer_handlers,
};
use crate::{
    ai_extractions, ai_governance, ai_suggestions, attachment_handlers, attachment_upload_handlers,
    audit, file_handlers, message_pins, message_search, state::AppState, tasks,
};

/// The canonical chat contract.
pub(crate) const CHAT_PREFIX: &str = "/api/chats";
/// Deprecated pre-TG-006 spelling, kept until M6 for the frozen Vue, PySide6 and ratatui
/// clients. `docs/tg/architecture.md` §5.1.
pub(crate) const DEPRECATED_CHAT_PREFIX: &str = "/api/rooms";

/// Every chat-scoped route, built once and mounted twice.
///
/// The deprecated `/api/rooms/*` alias is this same function with a different prefix and one
/// extra layer that marks the request's dialect. There is no second route list and no second
/// handler, so the alias cannot drift from the contract — which is a stronger guarantee than
/// the equivalence test alone gives.
fn chat_scoped_routes(prefix: &str, multipart_body_limit: usize) -> Router<Arc<AppState>> {
    let path = |suffix: &str| format!("{prefix}{suffix}");
    Router::new()
        .route(
            &path(""),
            get(chat_query_handlers::list_chats).post(handlers::create_chat),
        )
        .route(&path("/discover"), get(chat_query_handlers::discover_chats))
        .route(
            &path("/:id"),
            get(chat_query_handlers::get_chat)
                .patch(lifecycle_handlers::update_chat)
                .delete(lifecycle_handlers::delete_chat),
        )
        .route(&path("/:id/messages"), get(message_history::list_messages))
        .route(
            &path("/:id/draft"),
            get(drafts::get_draft).put(drafts::put_draft),
        )
        .route(
            &path("/:id/messages/search"),
            get(message_search::search_messages),
        )
        .route(
            &path("/:id/messages/:message_id/context"),
            get(message_search::message_context),
        )
        .route(
            &path("/:id/tasks"),
            get(tasks::handlers::list).post(tasks::handlers::create),
        )
        .route(
            &path("/:id/tasks/:task_id"),
            axum::routing::patch(tasks::handlers::update).delete(tasks::handlers::delete),
        )
        .route(&path("/:id/pins"), get(message_pins::list_pins))
        .route(
            &path("/:id/pins/:message_id"),
            axum::routing::post(message_pins::pin_message).delete(message_pins::unpin_message),
        )
        .route(&path("/:id/files"), get(file_handlers::list_chat_files))
        .route(
            &path("/:id/ai/suggest"),
            axum::routing::post(ai_suggestions::suggest),
        )
        .route(
            &path("/:id/ai/suggest/events"),
            axum::routing::post(ai_suggestions::suggest_events),
        )
        .route(
            &path("/:id/ai/extractions"),
            axum::routing::post(ai_extractions::handlers::create),
        )
        .route(&path("/:id/audit-events"), get(audit::handlers::list_chat))
        .route(
            &path("/:id/ai-policy"),
            get(ai_governance::handlers::chat_policy)
                .patch(ai_governance::handlers::update_chat_policy),
        )
        .route(
            &path("/:id/members"),
            get(membership_handlers::list_members),
        )
        .route(
            &path("/:id/members/me"),
            axum::routing::delete(membership_handlers::leave_chat)
                .patch(membership_handlers::update_own_nickname),
        )
        .route(
            &path("/:id/members/:user_id"),
            get(roster_handlers::get_member).patch(membership_handlers::update_member),
        )
        .route(
            &path("/:id/members/page"),
            get(roster_handlers::list_member_page),
        )
        .route(
            &path("/:id/members/:user_id/admin"),
            axum::routing::put(admin_handlers::put_admin).delete(admin_handlers::delete_admin),
        )
        .route(
            &path("/:id/members/:user_id/restrictions"),
            axum::routing::put(admin_handlers::put_restrictions),
        )
        .route(
            &path("/:id/permissions"),
            get(roster_handlers::get_permissions),
        )
        .route(
            &path("/:id/default-permissions"),
            axum::routing::put(roster_handlers::put_default_permissions),
        )
        .route(
            &path("/:id/join-requests"),
            axum::routing::post(membership_handlers::request_join),
        )
        .route(
            &path("/:id/invitations"),
            axum::routing::post(membership_handlers::invite_member),
        )
        .route(
            &path("/:id/attachments"),
            axum::routing::post(attachment_handlers::upload_attachment)
                .layer(axum::extract::DefaultBodyLimit::max(multipart_body_limit)),
        )
        .route(
            &path("/:id/username"),
            axum::routing::put(super::public_handles::put_username),
        )
        .route(
            &path("/:id/slow-mode"),
            get(slow_mode::get_slow_mode).put(slow_mode::put_slow_mode),
        )
        .route(
            &path("/:id/contact-messages"),
            axum::routing::post(crate::messages::contacts::send_contact),
        )
        .route(
            &path("/:id/notification-exception"),
            axum::routing::put(crate::notifications::exception_handlers::put_exception),
        )
        .route(
            &path("/:id/auto-delete"),
            axum::routing::put(crate::messages::auto_delete::put_auto_delete),
        )
        .route(
            &path("/:id/forum"),
            axum::routing::put(topic_handlers::put_forum),
        )
        .route(
            &path("/:id/topics"),
            get(topic_handlers::list_topics).post(topic_handlers::create_topic),
        )
        .route(
            &path("/:id/topics/:topic_id"),
            get(topic_handlers::get_topic)
                .patch(topic_handlers::update_topic)
                .delete(topic_handlers::delete_topic),
        )
        .route(
            &path("/:id/topics/:topic_id/messages"),
            get(topic_viewer_handlers::list_topic_messages),
        )
        .route(
            &path("/:id/topics/:topic_id/messages/:message_id/context"),
            get(topic_viewer_handlers::topic_message_context),
        )
        .route(
            &path("/:id/topics/:topic_id/read"),
            axum::routing::post(topic_viewer_handlers::read_topic),
        )
        .route(
            &path("/:id/topics/:topic_id/notifications"),
            axum::routing::put(topic_viewer_handlers::put_topic_notifications),
        )
        .route(
            &path("/:id/attachments/uploads"),
            axum::routing::post(attachment_upload_handlers::create_upload)
                .get(attachment_upload_handlers::list_uploads),
        )
}

/// `/api/chats/*` — the contract.
pub(crate) fn canonical(multipart_body_limit: usize) -> Router<Arc<AppState>> {
    chat_scoped_routes(CHAT_PREFIX, multipart_body_limit)
        // TG-202: channels exist only on the canonical prefix.
        .merge(super::channel_handlers::routes())
}

/// `/api/rooms/*` — the same tree, plus the one layer that tells the chat-descriptor handlers
/// to answer in the pre-rename dialect. Routing only: no handler is duplicated.
pub(crate) fn deprecated_alias(multipart_body_limit: usize) -> Router<Arc<AppState>> {
    chat_scoped_routes(DEPRECATED_CHAT_PREFIX, multipart_body_limit).layer(
        axum::middleware::from_fn(super::compat::mark_legacy_room_dialect),
    )
}
