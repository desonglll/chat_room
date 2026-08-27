//! Explicit room membership lifecycle endpoints.

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use uuid::Uuid;

use crate::admin_system_lock::require_room_unlocked;
use crate::audit::AuditEventDraft;
use crate::handlers::authorize_room;
use crate::models::{
    ChatMessage, InviteMemberRequest, JoinRoomRequest, RoomMembership, UpdateNicknameRequest,
};
use crate::state::SharedState;
use crate::user_handlers::bearer_token;

const MAX_NICKNAME_CHARS: usize = 48;

pub use super::governance_handlers::update_member;

pub(crate) async fn session_user(
    state: &SharedState,
    headers: &HeaderMap,
) -> Result<crate::models::User, StatusCode> {
    let token = bearer_token(headers)?;
    state
        .session_user(token)
        .await
        .map_err(|error| {
            tracing::error!("load membership session failed: {}", error);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or(StatusCode::UNAUTHORIZED)
}

pub(crate) async fn require_permission(
    state: &SharedState,
    room_id: Uuid,
    user_id: Uuid,
    permission: &str,
) -> Result<(), StatusCode> {
    state
        .has_room_permission(room_id, user_id, permission)
        .await
        .map_err(|error| {
            tracing::error!("check room permission failed: {}", error);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .then_some(())
        .ok_or(StatusCode::FORBIDDEN)
}

pub(crate) async fn reject_direct_room(
    state: &SharedState,
    room_id: Uuid,
) -> Result<(), StatusCode> {
    if state
        .is_direct_room(room_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    {
        return Err(StatusCode::NOT_FOUND);
    }
    Ok(())
}

pub(crate) async fn publish_membership_joined(
    state: &SharedState,
    room_id: Uuid,
    username: &str,
) -> Result<(), StatusCode> {
    let participants = state.room_participants(room_id).await.map_err(|error| {
        tracing::error!(
            "load participants after membership change failed: {}",
            error
        );
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    state
        .broadcast(
            room_id,
            ChatMessage::System {
                content: format!("{} joined the room", username),
                members: Some(state.connected_members(room_id).await),
                participants: Some(participants),
            },
        )
        .await;
    Ok(())
}

pub async fn request_join(
    State(state): State<SharedState>,
    Path(room_id): Path<Uuid>,
    headers: HeaderMap,
    Json(request): Json<JoinRoomRequest>,
) -> Result<(StatusCode, Json<RoomMembership>), StatusCode> {
    let user = session_user(&state, &headers).await?;
    require_room_unlocked(&state, room_id).await?;
    reject_direct_room(&state, room_id).await?;
    let room = state.room(room_id).await.ok_or(StatusCode::NOT_FOUND)?;
    if state
        .room_banned(room_id, user.id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    {
        return Err(StatusCode::FORBIDDEN);
    }
    if !authorize_room(&room, request.password.as_deref()) {
        return Err(StatusCode::UNAUTHORIZED);
    }
    let previous = state
        .membership_identity(room_id, user.id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let membership = state
        .request_room_membership(room_id, user.id, room.join_policy == "open")
        .await
        .map_err(|error| {
            tracing::error!("request room membership failed: {}", error);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    if membership.status == "active"
        && previous
            .as_ref()
            .is_none_or(|(status, _)| status != "active")
    {
        publish_membership_joined(&state, room_id, &user.username).await?;
    }
    let status = if membership.status == "active" {
        StatusCode::OK
    } else {
        StatusCode::ACCEPTED
    };
    Ok((status, Json(membership)))
}

pub async fn list_members(
    State(state): State<SharedState>,
    Path(room_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Vec<RoomMembership>>, StatusCode> {
    reject_direct_room(&state, room_id).await?;
    let user = session_user(&state, &headers).await?;
    require_permission(&state, room_id, user.id, "members.review").await?;
    let mut members = state.room_memberships(room_id).await.map_err(|error| {
        tracing::error!("list room memberships failed: {}", error);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    members.extend(
        state
            .banned_room_members(room_id)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
    );
    Ok(Json(members))
}

pub async fn invite_member(
    State(state): State<SharedState>,
    Path(room_id): Path<Uuid>,
    headers: HeaderMap,
    Json(request): Json<InviteMemberRequest>,
) -> Result<Json<RoomMembership>, StatusCode> {
    reject_direct_room(&state, room_id).await?;
    let user = session_user(&state, &headers).await?;
    require_permission(&state, room_id, user.id, "members.invite").await?;
    let username = request.username.trim();
    if username.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }
    state
        .record_audit_event(
            AuditEventDraft::room(&user, room_id, "room.member.invite_requested")
                .target("username", username),
        )
        .await
        .map_err(|error| {
            tracing::error!("required Room invitation audit failed: {error}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    state
        .invite_room_member(room_id, user.id, username)
        .await
        .map_err(|error| {
            tracing::error!("invite room member failed: {}", error);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

/// Set the caller's own nickname within one room. Any active member can do this —
/// unlike role/approval actions, it needs no management permission.
pub async fn update_own_nickname(
    State(state): State<SharedState>,
    Path(room_id): Path<Uuid>,
    headers: HeaderMap,
    Json(request): Json<UpdateNicknameRequest>,
) -> Result<Json<RoomMembership>, StatusCode> {
    reject_direct_room(&state, room_id).await?;
    let user = session_user(&state, &headers).await?;
    let nickname = request.nickname.trim();
    if nickname.chars().count() > MAX_NICKNAME_CHARS || nickname.chars().any(char::is_control) {
        return Err(StatusCode::BAD_REQUEST);
    }
    state
        .set_own_nickname(room_id, user.id, nickname)
        .await
        .map_err(|error| {
            tracing::error!("update own nickname failed: {}", error);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

/// Leave a room permanently until the account explicitly joins it again.
pub async fn leave_room(
    State(state): State<SharedState>,
    Path(room_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<StatusCode, StatusCode> {
    reject_direct_room(&state, room_id).await?;
    let user = session_user(&state, &headers).await?;
    if state.room(room_id).await.is_none() {
        return Err(StatusCode::NOT_FOUND);
    }
    let removed = state
        .delete_room_membership(room_id, user.id, true)
        .await
        .map_err(|error| {
            tracing::error!("remove room participant failed: {}", error);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    let Some(removed) = removed else {
        if state
            .room_membership(room_id, user.id)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
            .is_some_and(|membership| membership.role == "owner")
        {
            return Err(StatusCode::CONFLICT);
        }
        return Ok(StatusCode::NO_CONTENT);
    };

    let members = state.remove_connected_member(room_id, user.id).await;
    let participants = state.room_participants(room_id).await.map_err(|error| {
        tracing::error!("reload room participants after leave failed: {}", error);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    state
        .broadcast(
            room_id,
            ChatMessage::System {
                content: format!("{} left the room", removed.username),
                members: Some(members),
                participants: Some(participants),
            },
        )
        .await;
    state
        .disconnect_room_member(room_id, user.id, "membership left")
        .await;
    Ok(StatusCode::NO_CONTENT)
}
