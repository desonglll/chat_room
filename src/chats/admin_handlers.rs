//! Write surfaces of the permission system: appointing and dismissing administrators, and
//! restricting members. Each re-runs the authorization decision at write time and records a
//! required audit event before mutating.

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use chrono::Utc;
use uuid::Uuid;

use super::admin_models::{AdminAppointment, ChatMemberEntry, RestrictionWrite};
use super::admin_rights::MAX_CUSTOM_TITLE_CHARS;
use super::membership_handlers::{reject_private_chat, require_permission, session_user};
use super::permissions::{validate_keys, ADMIN_ASSIGNABLE, MEMBER_TOGGLEABLE};
use super::roster_handlers::{internal, member_entry};
use crate::audit::AuditEventDraft;
use crate::models::User;
use crate::state::SharedState;

/// The target's current role, or 404 when they are not an active member.
async fn target_role(
    state: &SharedState,
    room_id: Uuid,
    target_id: Uuid,
) -> Result<String, StatusCode> {
    state
        .member_row(room_id, target_id)
        .await
        .map_err(internal)?
        .map(|row| row.role)
        .ok_or(StatusCode::NOT_FOUND)
}

/// Only the owner (or the creator, or a system administrator) may change another
/// administrator. `members.roles` is the owner-only key that says so.
async fn manages_admins(
    state: &SharedState,
    room_id: Uuid,
    actor: &User,
) -> Result<bool, StatusCode> {
    state
        .has_chat_permission(room_id, actor.id, "members.roles")
        .await
        .map_err(internal)
}

async fn audit(state: &SharedState, draft: AuditEventDraft) -> Result<(), StatusCode> {
    state
        .record_audit_event(draft)
        .await
        .map(|_| ())
        .map_err(|error| {
            tracing::error!("required chat administration audit failed: {error}");
            StatusCode::INTERNAL_SERVER_ERROR
        })
}

async fn announced_entry(
    state: &SharedState,
    room_id: Uuid,
    target_id: Uuid,
    viewer_id: Uuid,
) -> Result<Json<ChatMemberEntry>, StatusCode> {
    state
        .announce_member(room_id, target_id)
        .await
        .map_err(internal)?;
    member_entry(state, room_id, target_id, viewer_id)
        .await?
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

#[utoipa::path(
    put,
    path = "/api/chats/{id}/members/{user_id}/admin",
    params(("id" = Uuid, description = "Chat id"), ("user_id" = Uuid, description = "Member")),
    request_body = AdminAppointment,
    responses(
        (status = 200, description = "Appointed (or rights edited)", body = ChatMemberEntry),
        (status = 400, description = "Unknown right or invalid title"),
        (status = 403, description = "Missing members.promote, or granting a right the actor lacks, or editing another admin without being the owner"),
        (status = 404, description = "Not an active member"),
        (status = 409, description = "Target is the owner or the actor")
    )
)]
pub async fn put_admin(
    State(state): State<SharedState>,
    Path((room_id, target_id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
    Json(request): Json<AdminAppointment>,
) -> Result<Json<ChatMemberEntry>, StatusCode> {
    reject_private_chat(&state, room_id).await?;
    let actor = session_user(&state, &headers).await?;
    require_permission(&state, room_id, actor.id, "members.promote").await?;
    let rights = validate_keys(&request.permissions, ADMIN_ASSIGNABLE)
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    let title = request.custom_title.trim();
    if title.chars().count() > MAX_CUSTOM_TITLE_CHARS || title.chars().any(char::is_control) {
        return Err(StatusCode::BAD_REQUEST);
    }
    if actor.id == target_id {
        return Err(StatusCode::CONFLICT);
    }
    let role = target_role(&state, room_id, target_id).await?;
    if role == "owner" {
        return Err(StatusCode::CONFLICT);
    }
    let owner_level = manages_admins(&state, room_id, &actor).await?;
    if role == "admin" && !owner_level {
        return Err(StatusCode::FORBIDDEN);
    }
    // Telegram's rule: nobody hands out a right they do not hold themselves.
    if !owner_level {
        for key in &rights {
            require_permission(&state, room_id, actor.id, key).await?;
        }
    }
    audit(
        &state,
        AuditEventDraft::chat(&actor, room_id, "room.member.promote_requested")
            .target("user", target_id)
            .detail("previous_role", &role)
            .detail("permissions", rights.join(",")),
    )
    .await?;
    if !state
        .appoint_chat_admin(room_id, target_id, &rights, title)
        .await
        .map_err(internal)?
    {
        return Err(StatusCode::CONFLICT);
    }
    announced_entry(&state, room_id, target_id, actor.id).await
}

#[utoipa::path(
    delete,
    path = "/api/chats/{id}/members/{user_id}/admin",
    params(("id" = Uuid, description = "Chat id"), ("user_id" = Uuid, description = "Administrator")),
    responses(
        (status = 200, description = "Dismissed, now an ordinary member", body = ChatMemberEntry),
        (status = 403, description = "Only the owner dismisses administrators"),
        (status = 404, description = "Not an administrator")
    )
)]
pub async fn delete_admin(
    State(state): State<SharedState>,
    Path((room_id, target_id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
) -> Result<Json<ChatMemberEntry>, StatusCode> {
    reject_private_chat(&state, room_id).await?;
    let actor = session_user(&state, &headers).await?;
    require_permission(&state, room_id, actor.id, "members.promote").await?;
    if !manages_admins(&state, room_id, &actor).await? {
        return Err(StatusCode::FORBIDDEN);
    }
    if target_role(&state, room_id, target_id).await? != "admin" {
        return Err(StatusCode::NOT_FOUND);
    }
    audit(
        &state,
        AuditEventDraft::chat(&actor, room_id, "room.member.dismiss_requested")
            .target("user", target_id),
    )
    .await?;
    if !state
        .dismiss_chat_admin(room_id, target_id)
        .await
        .map_err(internal)?
    {
        return Err(StatusCode::NOT_FOUND);
    }
    announced_entry(&state, room_id, target_id, actor.id).await
}

#[utoipa::path(
    put,
    path = "/api/chats/{id}/members/{user_id}/restrictions",
    params(("id" = Uuid, description = "Chat id"), ("user_id" = Uuid, description = "Member")),
    request_body = RestrictionWrite,
    responses(
        (status = 200, description = "Restrictions replaced (empty list lifts them)", body = ChatMemberEntry),
        (status = 400, description = "Unknown key or an `until` in the past"),
        (status = 403, description = "Missing members.ban"),
        (status = 404, description = "Not an active member"),
        (status = 409, description = "Target is the owner, an administrator, or the actor")
    )
)]
pub async fn put_restrictions(
    State(state): State<SharedState>,
    Path((room_id, target_id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
    Json(request): Json<RestrictionWrite>,
) -> Result<Json<ChatMemberEntry>, StatusCode> {
    reject_private_chat(&state, room_id).await?;
    let actor = session_user(&state, &headers).await?;
    require_permission(&state, room_id, actor.id, "members.ban").await?;
    let denied = validate_keys(&request.denied_permissions, MEMBER_TOGGLEABLE)
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    if request.until.is_some_and(|until| until <= Utc::now()) {
        return Err(StatusCode::BAD_REQUEST);
    }
    if actor.id == target_id {
        return Err(StatusCode::CONFLICT);
    }
    // An administrator is dismissed first, then restricted — never both at once.
    if target_role(&state, room_id, target_id).await? != "member" {
        return Err(StatusCode::CONFLICT);
    }
    let mut draft = AuditEventDraft::chat(&actor, room_id, "room.member.restrict_requested")
        .target("user", target_id)
        .detail("permissions", denied.join(","));
    if let Some(until) = request.until {
        draft = draft.detail("until", until.to_rfc3339());
    }
    audit(&state, draft).await?;
    state
        .replace_member_restrictions(room_id, target_id, &denied, request.until, actor.id)
        .await
        .map_err(internal)?;
    announced_entry(&state, room_id, target_id, actor.id).await
}
