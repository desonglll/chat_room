//! Link management over HTTP: `/api/chats/:id/invite-links/*`.
//!
//! Every request re-decides the management right (`store::LinkManager`). A link another
//! administrator created may be edited, revoked or deleted only by a holder of
//! `members.promote`; the primary link belongs to the chat, so any link manager may replace
//! or revoke it. Link changes are recorded in the chat's audit log without the token.

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use chrono::Utc;
use uuid::Uuid;

use super::store::{LinkManager, LinkWriteError};
use super::{InviteLink, InviteLinkMember, InviteLinkRequest, InviteLinksView};
use crate::audit::AuditEventDraft;
use crate::chats::membership_handlers::{reject_private_chat, session_user};
use crate::models::User;
use crate::state::SharedState;

fn internal(error: impl std::fmt::Display) -> StatusCode {
    tracing::error!("invite link request failed: {error}");
    StatusCode::INTERNAL_SERVER_ERROR
}

fn write_status(error: LinkWriteError) -> StatusCode {
    match error {
        LinkWriteError::NotFound => StatusCode::NOT_FOUND,
        LinkWriteError::Conflict => StatusCode::CONFLICT,
        LinkWriteError::Database(error) => internal(error),
    }
}

/// The session user and their management right, or 401/403/404.
async fn manager(
    state: &SharedState,
    room_id: Uuid,
    headers: &HeaderMap,
) -> Result<(User, LinkManager), StatusCode> {
    reject_private_chat(state, room_id).await?;
    let user = session_user(state, headers).await?;
    if state.chat(room_id).await.is_none() {
        return Err(StatusCode::NOT_FOUND);
    }
    let manager = state
        .invite_link_manager(room_id, user.id)
        .await
        .map_err(internal)?
        .ok_or(StatusCode::FORBIDDEN)?;
    Ok((user, manager))
}

/// The link, if the actor may change it.
async fn owned_link(
    state: &SharedState,
    room_id: Uuid,
    link_id: Uuid,
    actor: &User,
    manager: LinkManager,
) -> Result<InviteLink, StatusCode> {
    let link = state
        .chat_invite_link(room_id, link_id)
        .await
        .map_err(internal)?
        .ok_or(StatusCode::NOT_FOUND)?;
    if !link.is_primary && link.creator_id != Some(actor.id) && !manager.manages_others {
        return Err(StatusCode::FORBIDDEN);
    }
    Ok(link)
}

async fn audit(
    state: &SharedState,
    actor: &User,
    room_id: Uuid,
    event: &'static str,
    link_id: Uuid,
) -> Result<(), StatusCode> {
    state
        .record_audit_event(
            AuditEventDraft::chat(actor, room_id, event).target("invite_link", link_id),
        )
        .await
        .map(|_| ())
        .map_err(internal)
}

#[utoipa::path(
    get,
    path = "/api/chats/{id}/invite-links",
    params(("id" = Uuid, description = "Chat id")),
    responses(
        (status = 200, description = "Every link of the chat; a primary link is issued on first read", body = InviteLinksView),
        (status = 403, description = "Not an administrator holding members.invite"),
        (status = 404, description = "Chat not found or private")
    )
)]
pub async fn list_links(
    State(state): State<SharedState>,
    Path(room_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<InviteLinksView>, StatusCode> {
    let (user, manager) = manager(&state, room_id, &headers).await?;
    state
        .ensure_primary_invite_link(room_id, user.id)
        .await
        .map_err(internal)?;
    let links = state.chat_invite_links(room_id).await.map_err(internal)?;
    Ok(Json(InviteLinksView {
        links,
        can_review: manager.reviews,
        can_manage_others: manager.manages_others,
    }))
}

#[utoipa::path(
    post,
    path = "/api/chats/{id}/invite-links",
    params(("id" = Uuid, description = "Chat id")),
    request_body = InviteLinkRequest,
    responses(
        (status = 201, description = "Additional link created", body = InviteLink),
        (status = 400, description = "Invalid title, expiry or limit"),
        (status = 403, description = "Not an administrator holding members.invite")
    )
)]
pub async fn create_link(
    State(state): State<SharedState>,
    Path(room_id): Path<Uuid>,
    headers: HeaderMap,
    Json(request): Json<InviteLinkRequest>,
) -> Result<(StatusCode, Json<InviteLink>), StatusCode> {
    let (user, _) = manager(&state, room_id, &headers).await?;
    let settings = request
        .validate(Utc::now())
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    let link = state
        .create_invite_link(room_id, user.id, &settings)
        .await
        .map_err(internal)?;
    audit(&state, &user, room_id, "room.invite_link.created", link.id).await?;
    Ok((StatusCode::CREATED, Json(link)))
}

#[utoipa::path(
    put,
    path = "/api/chats/{id}/invite-links/{link_id}",
    params(("id" = Uuid, description = "Chat id"), ("link_id" = Uuid, description = "Link id")),
    request_body = InviteLinkRequest,
    responses(
        (status = 200, description = "Settings replaced; the token is unchanged", body = InviteLink),
        (status = 400, description = "Invalid title, expiry or limit"),
        (status = 403, description = "Not a link manager, or another administrator's link without members.promote"),
        (status = 409, description = "The link is revoked or primary")
    )
)]
pub async fn edit_link(
    State(state): State<SharedState>,
    Path((room_id, link_id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
    Json(request): Json<InviteLinkRequest>,
) -> Result<Json<InviteLink>, StatusCode> {
    let (user, manager) = manager(&state, room_id, &headers).await?;
    owned_link(&state, room_id, link_id, &user, manager).await?;
    let settings = request
        .validate(Utc::now())
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    let link = state
        .edit_invite_link(room_id, link_id, &settings)
        .await
        .map_err(write_status)?;
    audit(&state, &user, room_id, "room.invite_link.edited", link_id).await?;
    Ok(Json(link))
}

#[utoipa::path(
    post,
    path = "/api/chats/{id}/invite-links/{link_id}/revoke",
    params(("id" = Uuid, description = "Chat id"), ("link_id" = Uuid, description = "Link id")),
    responses(
        (status = 200, description = "Revoked, effective for the next join; revoking the primary link issues a new one", body = InviteLink),
        (status = 403, description = "Not a link manager, or another administrator's link without members.promote"),
        (status = 404, description = "No such link in this chat")
    )
)]
pub async fn revoke_link(
    State(state): State<SharedState>,
    Path((room_id, link_id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
) -> Result<Json<InviteLink>, StatusCode> {
    let (user, manager) = manager(&state, room_id, &headers).await?;
    owned_link(&state, room_id, link_id, &user, manager).await?;
    let link = state
        .revoke_invite_link(room_id, link_id, user.id)
        .await
        .map_err(write_status)?;
    audit(&state, &user, room_id, "room.invite_link.revoked", link_id).await?;
    Ok(Json(link))
}

#[utoipa::path(
    delete,
    path = "/api/chats/{id}/invite-links/{link_id}",
    params(("id" = Uuid, description = "Chat id"), ("link_id" = Uuid, description = "Link id")),
    responses(
        (status = 204, description = "Revoked link deleted"),
        (status = 409, description = "The link is still live; revoke it first")
    )
)]
pub async fn delete_link(
    State(state): State<SharedState>,
    Path((room_id, link_id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
) -> Result<StatusCode, StatusCode> {
    let (user, manager) = manager(&state, room_id, &headers).await?;
    owned_link(&state, room_id, link_id, &user, manager).await?;
    state
        .delete_invite_link(room_id, link_id)
        .await
        .map_err(write_status)?;
    audit(&state, &user, room_id, "room.invite_link.deleted", link_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    post,
    path = "/api/chats/{id}/invite-links/primary",
    params(("id" = Uuid, description = "Chat id")),
    responses(
        (status = 200, description = "The old primary link is revoked and this new one replaces it", body = InviteLink),
        (status = 403, description = "Not an administrator holding members.invite")
    )
)]
pub async fn replace_primary(
    State(state): State<SharedState>,
    Path(room_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<InviteLink>, StatusCode> {
    let (user, _) = manager(&state, room_id, &headers).await?;
    let link = state
        .replace_primary_invite_link(room_id, user.id)
        .await
        .map_err(internal)?;
    audit(&state, &user, room_id, "room.invite_link.replaced", link.id).await?;
    Ok(Json(link))
}

async fn link_people(
    state: SharedState,
    room_id: Uuid,
    link_id: Uuid,
    headers: HeaderMap,
    status: &str,
) -> Result<Json<Vec<InviteLinkMember>>, StatusCode> {
    manager(&state, room_id, &headers).await?;
    state
        .chat_invite_link(room_id, link_id)
        .await
        .map_err(internal)?
        .ok_or(StatusCode::NOT_FOUND)?;
    state
        .invite_link_members(room_id, link_id, status)
        .await
        .map(Json)
        .map_err(internal)
}

#[utoipa::path(
    get,
    path = "/api/chats/{id}/invite-links/{link_id}/members",
    params(("id" = Uuid, description = "Chat id"), ("link_id" = Uuid, description = "Link id")),
    responses(
        (status = 200, description = "Current members who joined through this link, newest first (≤200)", body = [InviteLinkMember]),
        (status = 403, description = "Not an administrator holding members.invite")
    )
)]
pub async fn link_members(
    State(state): State<SharedState>,
    Path((room_id, link_id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
) -> Result<Json<Vec<InviteLinkMember>>, StatusCode> {
    link_people(state, room_id, link_id, headers, "active").await
}

#[utoipa::path(
    get,
    path = "/api/chats/{id}/invite-links/{link_id}/requests",
    params(("id" = Uuid, description = "Chat id"), ("link_id" = Uuid, description = "Link id")),
    responses(
        (status = 200, description = "Pending join requests that arrived through this link; settle them with PATCH /members/{user_id} approve|reject", body = [InviteLinkMember]),
        (status = 403, description = "Not an administrator holding members.invite")
    )
)]
pub async fn link_requests(
    State(state): State<SharedState>,
    Path((room_id, link_id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
) -> Result<Json<Vec<InviteLinkMember>>, StatusCode> {
    link_people(state, room_id, link_id, headers, "pending").await
}
