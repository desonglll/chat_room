//! Explicit room membership lifecycle endpoints.

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use uuid::Uuid;

use crate::handlers::authorize_room;
use crate::models::{
    ChatMessage, InviteMemberRequest, JoinRoomRequest, RoomMembership, UpdateMembershipRequest,
};
use crate::state::SharedState;
use crate::user_handlers::bearer_token;

async fn session_user(
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

async fn require_permission(
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

async fn publish_membership_joined(
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
    let room = state.room(room_id).await.ok_or(StatusCode::NOT_FOUND)?;
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
    let user = session_user(&state, &headers).await?;
    require_permission(&state, room_id, user.id, "members.review").await?;
    state
        .room_memberships(room_id)
        .await
        .map(Json)
        .map_err(|error| {
            tracing::error!("list room memberships failed: {}", error);
            StatusCode::INTERNAL_SERVER_ERROR
        })
}

pub async fn invite_member(
    State(state): State<SharedState>,
    Path(room_id): Path<Uuid>,
    headers: HeaderMap,
    Json(request): Json<InviteMemberRequest>,
) -> Result<Json<RoomMembership>, StatusCode> {
    let user = session_user(&state, &headers).await?;
    require_permission(&state, room_id, user.id, "members.invite").await?;
    let username = request.username.trim();
    if username.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }
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

pub async fn update_member(
    State(state): State<SharedState>,
    Path((room_id, target_id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
    Json(request): Json<UpdateMembershipRequest>,
) -> Result<Json<RoomMembership>, StatusCode> {
    let actor = session_user(&state, &headers).await?;
    let permission = match request.action.as_str() {
        "approve" | "reject" => "members.review",
        "remove" => "members.remove",
        "set_role" => "members.roles",
        _ => return Err(StatusCode::BAD_REQUEST),
    };
    require_permission(&state, room_id, actor.id, permission).await?;
    let previous = state
        .room_membership(room_id, target_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    if previous.role == "owner" || (request.action == "remove" && actor.id == target_id) {
        return Err(StatusCode::CONFLICT);
    }

    let result = match request.action.as_str() {
        "approve" => state.activate_room_member(room_id, target_id).await,
        "set_role" => {
            let role = request.role.as_deref().ok_or(StatusCode::BAD_REQUEST)?;
            state.set_room_member_role(room_id, target_id, role).await
        }
        "reject" | "remove" => {
            state
                .delete_room_membership(room_id, target_id, false)
                .await
        }
        _ => unreachable!(),
    };
    let updated = result
        .map_err(|error| {
            tracing::error!("update room membership failed: {}", error);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or(StatusCode::CONFLICT)?;

    if request.action == "approve" {
        publish_membership_joined(&state, room_id, &updated.username).await?;
    } else if request.action == "remove" {
        let members = state.remove_connected_member(room_id, target_id).await;
        let participants = state
            .room_participants(room_id)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        state
            .broadcast(
                room_id,
                ChatMessage::System {
                    content: format!("{} was removed from the room", updated.username),
                    members: Some(members),
                    participants: Some(participants),
                },
            )
            .await;
        state
            .disconnect_room_member(room_id, target_id, "membership removed")
            .await;
    }
    Ok(Json(updated))
}

/// Leave a room permanently until the account explicitly joins it again.
pub async fn leave_room(
    State(state): State<SharedState>,
    Path(room_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<StatusCode, StatusCode> {
    let user = session_user(&state, &headers).await?;
    if state.room(room_id).await.is_none() {
        return Err(StatusCode::NOT_FOUND);
    }
    let membership = state
        .room_membership(room_id, user.id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if membership
        .as_ref()
        .is_some_and(|membership| membership.role == "owner")
    {
        return Err(StatusCode::CONFLICT);
    }
    let removed = state
        .remove_room_participant(room_id, user.id)
        .await
        .map_err(|error| {
            tracing::error!("remove room participant failed: {}", error);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    if !removed {
        return Ok(StatusCode::NO_CONTENT);
    }

    let members = state.remove_connected_member(room_id, user.id).await;
    let participants = state.room_participants(room_id).await.map_err(|error| {
        tracing::error!("reload room participants after leave failed: {}", error);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    state
        .broadcast(
            room_id,
            ChatMessage::System {
                content: format!("{} left the room", user.username),
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
