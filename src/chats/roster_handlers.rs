//! Read surfaces of the permission system: the viewer's permissions, the group's defaults,
//! and the paged roster. Every handler re-authorizes on each request (read-time check).

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use chrono::Utc;
use serde::Deserialize;
use uuid::Uuid;

use super::admin_models::{
    ChatMemberEntry, ChatMemberPage, ChatPermissionsView, DefaultPermissionsWrite,
};
use super::member_page::{
    MemberCursor, MemberFilter, RosterVisibility, DEFAULT_PAGE_SIZE, MAX_PAGE_SIZE,
};
use super::membership_handlers::{reject_private_chat, require_permission, session_user};
use super::permissions::{validate_keys, MEMBER_TOGGLEABLE};
use super::ChatType;
use crate::audit::AuditEventDraft;
use crate::models::ChatMessage;
use crate::state::SharedState;

pub(crate) fn internal(error: sqlx::Error) -> StatusCode {
    tracing::error!("chat administration query failed: {error}");
    StatusCode::INTERNAL_SERVER_ERROR
}

/// The read gate for everything a member may see: an active membership.
pub(crate) async fn require_reader(
    state: &SharedState,
    room_id: Uuid,
    user_id: Uuid,
) -> Result<(), StatusCode> {
    state
        .can_read_chat(room_id, user_id)
        .await
        .map_err(internal)?
        .then_some(())
        .ok_or(StatusCode::FORBIDDEN)
}

pub(crate) async fn roster_visibility(
    state: &SharedState,
    room_id: Uuid,
    viewer_id: Uuid,
) -> Result<RosterVisibility, StatusCode> {
    Ok(RosterVisibility {
        viewer_id,
        sees_admin_rights: state
            .has_chat_permission(room_id, viewer_id, "members.promote")
            .await
            .map_err(internal)?,
        sees_restrictions: state
            .has_chat_permission(room_id, viewer_id, "members.ban")
            .await
            .map_err(internal)?,
    })
}

#[utoipa::path(
    get,
    path = "/api/chats/{id}/permissions",
    params(("id" = Uuid, description = "Chat id")),
    responses(
        (status = 200, description = "Default permissions, the viewer's effective permissions and the registry", body = ChatPermissionsView),
        (status = 403, description = "Not an active member"),
        (status = 404, description = "Chat not found or private")
    )
)]
pub async fn get_permissions(
    State(state): State<SharedState>,
    Path(room_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<ChatPermissionsView>, StatusCode> {
    reject_private_chat(&state, room_id).await?;
    let user = session_user(&state, &headers).await?;
    require_reader(&state, room_id, user.id).await?;
    state
        .chat_permissions_view(room_id, user.id)
        .await
        .map_err(internal)?
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

#[utoipa::path(
    put,
    path = "/api/chats/{id}/default-permissions",
    params(("id" = Uuid, description = "Chat id")),
    request_body = DefaultPermissionsWrite,
    responses(
        (status = 200, description = "Defaults replaced", body = ChatPermissionsView),
        (status = 400, description = "A key outside the member-toggleable set"),
        (status = 403, description = "Missing members.ban"),
        (status = 404, description = "Chat not found or private")
    )
)]
pub async fn put_default_permissions(
    State(state): State<SharedState>,
    Path(room_id): Path<Uuid>,
    headers: HeaderMap,
    Json(request): Json<DefaultPermissionsWrite>,
) -> Result<Json<ChatPermissionsView>, StatusCode> {
    reject_private_chat(&state, room_id).await?;
    let user = session_user(&state, &headers).await?;
    require_permission(&state, room_id, user.id, "members.ban").await?;
    let enabled = validate_keys(&request.permissions, MEMBER_TOGGLEABLE)
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    state
        .record_audit_event(
            AuditEventDraft::chat(&user, room_id, "room.permissions.defaults_change_requested")
                .detail("permissions", enabled.join(",")),
        )
        .await
        .map_err(|error| {
            tracing::error!("required default permissions audit failed: {error}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    state
        .replace_chat_default_permissions(room_id, &enabled)
        .await
        .map_err(internal)?;
    let view = state
        .chat_permissions_view(room_id, user.id)
        .await
        .map_err(internal)?
        .ok_or(StatusCode::NOT_FOUND)?;
    if let Some(mut chat) = state.chat(room_id).await {
        chat.membership_status = None;
        chat.membership_role = None;
        state
            .broadcast(room_id, ChatMessage::ChatUpdated { chat })
            .await;
    }
    Ok(Json(view))
}

#[derive(Debug, Deserialize)]
pub struct MemberPageQuery {
    pub cursor: Option<String>,
    pub limit: Option<i64>,
    pub filter: Option<String>,
}

#[utoipa::path(
    get,
    path = "/api/chats/{id}/members/page",
    params(
        ("id" = Uuid, description = "Chat id"),
        ("cursor" = Option<String>, Query, description = "next_cursor of the previous page"),
        ("limit" = Option<i64>, Query, description = "1-200, default 50"),
        ("filter" = Option<String>, Query, description = "all (default), admins or restricted")
    ),
    responses(
        (status = 200, description = "One keyset page, newest joiner first", body = ChatMemberPage),
        (status = 400, description = "Malformed cursor, limit or filter"),
        (status = 403, description = "Not an active member (a channel's roster needs members.review)"),
        (status = 404, description = "Chat not found or private")
    )
)]
pub async fn list_member_page(
    State(state): State<SharedState>,
    Path(room_id): Path<Uuid>,
    Query(query): Query<MemberPageQuery>,
    headers: HeaderMap,
) -> Result<Json<ChatMemberPage>, StatusCode> {
    reject_private_chat(&state, room_id).await?;
    let user = session_user(&state, &headers).await?;
    require_reader(&state, room_id, user.id).await?;
    // Telegram hides a channel's subscriber list from subscribers.
    if state.chat(room_id).await.map(|chat| chat.chat_type) == Some(ChatType::Channel) {
        require_permission(&state, room_id, user.id, "members.review").await?;
    }
    let filter = match query.filter.as_deref() {
        None => MemberFilter::All,
        Some(value) => value.parse().map_err(|_| StatusCode::BAD_REQUEST)?,
    };
    let after = match query.cursor.as_deref() {
        None | Some("") => None,
        Some(value) => Some(MemberCursor::decode(value).ok_or(StatusCode::BAD_REQUEST)?),
    };
    let limit = query.limit.unwrap_or(DEFAULT_PAGE_SIZE);
    if !(1..=MAX_PAGE_SIZE).contains(&limit) {
        return Err(StatusCode::BAD_REQUEST);
    }
    let visibility = roster_visibility(&state, room_id, user.id).await?;
    state
        .chat_member_page(room_id, filter, after, limit, visibility)
        .await
        .map(Json)
        .map_err(internal)
}

#[utoipa::path(
    get,
    path = "/api/chats/{id}/members/{user_id}",
    params(("id" = Uuid, description = "Chat id"), ("user_id" = Uuid, description = "Member")),
    responses(
        (status = 200, description = "One active member", body = ChatMemberEntry),
        (status = 403, description = "Not an active member"),
        (status = 404, description = "No such active member")
    )
)]
pub async fn get_member(
    State(state): State<SharedState>,
    Path((room_id, target_id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
) -> Result<Json<ChatMemberEntry>, StatusCode> {
    reject_private_chat(&state, room_id).await?;
    let user = session_user(&state, &headers).await?;
    require_reader(&state, room_id, user.id).await?;
    member_entry(&state, room_id, target_id, user.id)
        .await?
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

/// One member as `viewer_id` may see them, or `None` when not an active member.
pub(crate) async fn member_entry(
    state: &SharedState,
    room_id: Uuid,
    target_id: Uuid,
    viewer_id: Uuid,
) -> Result<Option<ChatMemberEntry>, StatusCode> {
    let Some(row) = state
        .member_row(room_id, target_id)
        .await
        .map_err(internal)?
    else {
        return Ok(None);
    };
    let visibility = roster_visibility(state, room_id, viewer_id).await?;
    Ok(state
        .decorate_member_rows(room_id, vec![row], visibility, Utc::now())
        .await
        .map_err(internal)?
        .pop())
}
