//! Batch-forward previously sent messages into other chats.

use axum::{extract::State, http::StatusCode, Json};

use crate::models::{ForwardMessagesRequest, ForwardResult};
use crate::realtime::protocol::stored_message_to_chat;
use crate::state::SharedState;
use crate::user_handlers::bearer_token;

/// Forward one or more messages into one or more target chats. Each
/// (source message, target chat) pair is attempted independently — a message
/// that has since been recalled, or a chat the caller can no longer post in,
/// is skipped rather than failing the whole batch.
#[utoipa::path(
    post,
    path = "/api/messages/forward",
    request_body = ForwardMessagesRequest,
    responses(
        (status = 200, description = "Per-pair forward outcome", body = Vec<ForwardResult>),
        (status = 401, description = "Missing or expired session")
    )
)]
pub async fn forward_messages(
    State(state): State<SharedState>,
    headers: axum::http::HeaderMap,
    Json(request): Json<ForwardMessagesRequest>,
) -> Result<Json<Vec<ForwardResult>>, StatusCode> {
    let token = bearer_token(&headers)?;
    let user = state
        .session_user(token)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::UNAUTHORIZED)?;

    let mut results = Vec::with_capacity(request.message_ids.len() * request.target_room_ids.len());
    // TG-403: oldest first, and album items forwarded together stay an album in the target.
    let mut plan = state.plan_forward(&request.message_ids).await.map_err(|error| {
        tracing::error!("plan forward failed: {}", error);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    for message_id in std::mem::take(&mut plan.order) {
        let Some(source_room_id) = state.message_room_id(message_id).await.map_err(|error| {
            tracing::error!("look up source chat for forward failed: {}", error);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        else {
            for &target_room_id in &request.target_room_ids {
                results.push(ForwardResult {
                    message_id,
                    target_room_id,
                    forwarded_message_id: None,
                    skipped_reason: Some("message not found".into()),
                });
            }
            continue;
        };
        // TG-201: reading the source needs membership, not the right to send there.
        let is_source_member = state
            .can_read_chat(source_room_id, user.id)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        if !is_source_member {
            for &target_room_id in &request.target_room_ids {
                results.push(ForwardResult {
                    message_id,
                    target_room_id,
                    forwarded_message_id: None,
                    skipped_reason: Some("not a member of the source chat".into()),
                });
            }
            continue;
        }
        for &target_room_id in &request.target_room_ids {
            let can_send = state
                .has_chat_permission(target_room_id, user.id, "message.send")
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            if !can_send {
                results.push(ForwardResult {
                    message_id,
                    target_room_id,
                    forwarded_message_id: None,
                    skipped_reason: Some("cannot send to the target chat".into()),
                });
                continue;
            }
            // TG-204: a forward lands in a forum's General topic, which may be closed.
            if state
                .resolve_post_topic(target_room_id, user.id, None)
                .await
                .is_err()
            {
                results.push(ForwardResult {
                    message_id,
                    target_room_id,
                    forwarded_message_id: None,
                    skipped_reason: Some("the target topic is closed".into()),
                });
                continue;
            }
            let Ok(_permit) = state.work_queue().message().await else {
                results.push(ForwardResult {
                    message_id,
                    target_room_id,
                    forwarded_message_id: None,
                    skipped_reason: Some("server busy; retry this forward".into()),
                });
                continue;
            };
            let grouped_id = plan.target_group(message_id, target_room_id);
            match state
                .forward_message_grouped(
                    message_id,
                    source_room_id,
                    target_room_id,
                    &user,
                    grouped_id,
                )
                .await
            {
                Ok(Some(forwarded)) => {
                    results.push(ForwardResult {
                        message_id,
                        target_room_id,
                        forwarded_message_id: Some(forwarded.id),
                        skipped_reason: None,
                    });
                    state
                        .broadcast(target_room_id, stored_message_to_chat(forwarded))
                        .await;
                }
                Ok(None) => {
                    results.push(ForwardResult {
                        message_id,
                        target_room_id,
                        forwarded_message_id: None,
                        skipped_reason: Some("message was recalled".into()),
                    });
                }
                Err(error) => {
                    tracing::error!("forward message failed: {}", error);
                    results.push(ForwardResult {
                        message_id,
                        target_room_id,
                        forwarded_message_id: None,
                        skipped_reason: Some("forward failed".into()),
                    });
                }
            }
        }
    }
    Ok(Json(results))
}
