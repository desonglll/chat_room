//! Audited Room membership governance actions.

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use uuid::Uuid;

use crate::{
    audit::AuditEventDraft,
    models::{ChatMessage, RoomMembership, UpdateMembershipRequest},
    state::SharedState,
};

use super::membership_handlers::{
    publish_membership_joined, reject_direct_room, require_permission, session_user,
};

pub async fn update_member(
    State(state): State<SharedState>,
    Path((room_id, target_id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
    Json(request): Json<UpdateMembershipRequest>,
) -> Result<Json<RoomMembership>, StatusCode> {
    reject_direct_room(&state, room_id).await?;
    let actor = session_user(&state, &headers).await?;
    let permission = match request.action.as_str() {
        "approve" | "reject" => "members.review",
        "remove" | "ban" | "unban" => "members.remove",
        "set_role" => "members.roles",
        _ => return Err(StatusCode::BAD_REQUEST),
    };
    require_permission(&state, room_id, actor.id, permission).await?;
    let previous = if request.action == "unban" {
        state.room_ban_membership(room_id, target_id).await
    } else {
        state.room_membership(room_id, target_id).await
    }
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::NOT_FOUND)?;
    if previous.role == "owner"
        || (matches!(request.action.as_str(), "remove" | "ban") && actor.id == target_id)
    {
        return Err(StatusCode::CONFLICT);
    }

    let event_type = event_type(&request.action)?;
    let mut audit = AuditEventDraft::room(&actor, room_id, event_type)
        .target("user", target_id)
        .detail("previous_status", &previous.status)
        .detail("previous_role", &previous.role);
    if let Some(role) = request.role.as_deref() {
        audit = audit.detail("role", role);
    }
    state.record_audit_event(audit).await.map_err(|error| {
        tracing::error!("required Room governance audit failed: {error}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let updated = mutate(&state, room_id, target_id, actor.id, &request)
        .await?
        .ok_or(StatusCode::CONFLICT)?;
    if request.action == "approve" {
        publish_membership_joined(&state, room_id, &updated.username).await?;
    } else if matches!(request.action.as_str(), "remove" | "ban") {
        disconnect_removed(&state, room_id, target_id, &updated, &request.action).await?;
    }
    Ok(Json(updated))
}

fn event_type(action: &str) -> Result<&'static str, StatusCode> {
    match action {
        "approve" => Ok("room.member.approve_requested"),
        "reject" => Ok("room.member.reject_requested"),
        "remove" => Ok("room.member.remove_requested"),
        "set_role" => Ok("room.member.role_change_requested"),
        "ban" => Ok("room.member.ban_requested"),
        "unban" => Ok("room.member.unban_requested"),
        _ => Err(StatusCode::BAD_REQUEST),
    }
}

async fn mutate(
    state: &SharedState,
    room_id: Uuid,
    target_id: Uuid,
    actor_id: Uuid,
    request: &UpdateMembershipRequest,
) -> Result<Option<RoomMembership>, StatusCode> {
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
        "ban" => state.ban_room_member(room_id, target_id, actor_id).await,
        "unban" => state.unban_room_member(room_id, target_id).await,
        _ => unreachable!(),
    };
    result.map_err(|error| {
        tracing::error!("update room membership failed: {error}");
        StatusCode::INTERNAL_SERVER_ERROR
    })
}

async fn disconnect_removed(
    state: &SharedState,
    room_id: Uuid,
    target_id: Uuid,
    updated: &RoomMembership,
    action: &str,
) -> Result<(), StatusCode> {
    let banned = action == "ban";
    let members = state.remove_connected_member(room_id, target_id).await;
    let participants = state
        .room_participants(room_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    state
        .broadcast(
            room_id,
            ChatMessage::System {
                content: format!(
                    "{} was {} from the room",
                    updated.username,
                    if banned { "banned" } else { "removed" }
                ),
                members: Some(members),
                participants: Some(participants),
            },
        )
        .await;
    state
        .disconnect_room_member(
            room_id,
            target_id,
            if banned {
                "membership banned"
            } else {
                "membership removed"
            },
        )
        .await;
    Ok(())
}
