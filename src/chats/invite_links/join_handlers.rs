//! The link holder's side: `/api/invite-links/:token` (preview) and `/join`.
//!
//! A refusal answers with a JSON `{ "error": <reason> }` so the landing page can say why:
//! `not_found` (404), `expired` / `limit_reached` / `revoked` (410), `banned` (403), `locked` (423).
//! The token is never logged.

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

use super::join::{InviteJoinError, InviteJoinOutcome, InviteRejection};
use super::{plausible_token, InviteJoinResult};
use crate::chats::membership_handlers::{publish_membership_joined, session_user};
use crate::state::SharedState;

fn refusal(error: InviteJoinError) -> Response {
    let (status, reason) = match error {
        InviteJoinError::Rejected(InviteRejection::NotFound) => {
            (StatusCode::NOT_FOUND, "not_found")
        }
        InviteJoinError::Rejected(InviteRejection::Unusable(state)) => {
            (StatusCode::GONE, state.as_str())
        }
        InviteJoinError::Rejected(InviteRejection::Banned) => (StatusCode::FORBIDDEN, "banned"),
        InviteJoinError::Rejected(InviteRejection::Locked) => (StatusCode::LOCKED, "locked"),
        InviteJoinError::Database(error) => {
            tracing::error!("invite link join failed: {error}");
            (StatusCode::INTERNAL_SERVER_ERROR, "internal")
        }
    };
    (status, Json(json!({ "error": reason }))).into_response()
}

fn not_found() -> Response {
    refusal(InviteJoinError::Rejected(InviteRejection::NotFound))
}

#[utoipa::path(
    get,
    path = "/api/invite-links/{token}",
    params(("token" = String, description = "Invite link token")),
    responses(
        (status = 200, description = "Chat preview for a usable link", body = super::InvitePreview),
        (status = 401, description = "No session"),
        (status = 404, description = "Unknown link"),
        (status = 410, description = "expired, limit_reached or revoked")
    )
)]
pub async fn preview(
    State(state): State<SharedState>,
    Path(token): Path<String>,
    headers: HeaderMap,
) -> Response {
    let user = match session_user(&state, &headers).await {
        Ok(user) => user,
        Err(status) => return status.into_response(),
    };
    if !plausible_token(&token) {
        return not_found();
    }
    match state.invite_preview(&token, user.id).await {
        Ok(preview) => Json(preview).into_response(),
        Err(error) => refusal(error),
    }
}

#[utoipa::path(
    post,
    path = "/api/invite-links/{token}/join",
    params(("token" = String, description = "Invite link token")),
    responses(
        (status = 200, description = "Joined (or already a member)", body = InviteJoinResult),
        (status = 202, description = "Join request queued for approval", body = InviteJoinResult),
        (status = 403, description = "banned"),
        (status = 404, description = "Unknown link"),
        (status = 410, description = "expired, limit_reached or revoked"),
        (status = 423, description = "Chat locked by a system administrator")
    )
)]
pub async fn join(
    State(state): State<SharedState>,
    Path(token): Path<String>,
    headers: HeaderMap,
) -> Response {
    let user = match session_user(&state, &headers).await {
        Ok(user) => user,
        Err(status) => return status.into_response(),
    };
    if !plausible_token(&token) {
        return not_found();
    }
    let outcome = match state.join_by_invite_link(&token, user.id).await {
        Ok(outcome) => outcome,
        Err(error) => return refusal(error),
    };
    let (status, result) = match outcome {
        InviteJoinOutcome::Joined { chat_id } => {
            if let Err(status) = publish_membership_joined(&state, chat_id, &user.username).await {
                return status.into_response();
            }
            (StatusCode::OK, ("active", Some(chat_id)))
        }
        InviteJoinOutcome::AlreadyMember { chat_id } => (StatusCode::OK, ("active", Some(chat_id))),
        InviteJoinOutcome::Requested { .. } => (StatusCode::ACCEPTED, ("pending", None)),
    };
    (
        status,
        Json(InviteJoinResult {
            status: result.0.to_string(),
            chat_id: result.1,
        }),
    )
        .into_response()
}
