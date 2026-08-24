//! WebSocket enforcement for the persistent system chat lock.

use axum::extract::ws::{Message, WebSocket};
use futures_util::{stream::SplitSink, SinkExt};

use super::ws::send_json;
use uuid::Uuid;

use crate::admin_system_lock::room_lock_reason;
use crate::models::ChatMessage;
use crate::state::SharedState;

pub(super) async fn reject_locked_auth(
    state: &SharedState,
    room_id: Uuid,
    sink: &mut SplitSink<WebSocket, Message>,
) -> bool {
    let reason = match room_lock_reason(state, room_id).await {
        Ok(None) => return false,
        Ok(Some(reason)) => reason,
        Err(error) => {
            tracing::error!("read system chat lock during authentication failed: {error}");
            "authentication unavailable"
        }
    };
    let _ = send_json(
        sink,
        &ChatMessage::AuthFail {
            reason: reason.into(),
        },
    )
    .await;
    true
}

pub(super) async fn close_if_locked(
    state: &SharedState,
    room_id: Uuid,
    sink: &mut SplitSink<WebSocket, Message>,
) -> bool {
    match room_lock_reason(state, room_id).await {
        Ok(Some(reason)) => {
            let _ = send_json(
                sink,
                &ChatMessage::System {
                    content: reason.into(),
                    members: None,
                    participants: None,
                },
            )
            .await;
            let _ = sink.close().await;
            true
        }
        Ok(None) => false,
        Err(error) => {
            tracing::warn!("check system chat lock failed: {error}");
            false
        }
    }
}
