//! WebSocket enforcement for the persistent system chat lock.

use axum::extract::ws::{Message, WebSocket};
use futures_util::{stream::SplitSink, SinkExt};

use super::ws::send_json;
use crate::admin_system_lock::SYSTEM_LOCK_REASON;
use crate::models::ChatMessage;
use crate::state::SharedState;

pub(super) async fn reject_locked_auth(
    state: &SharedState,
    sink: &mut SplitSink<WebSocket, Message>,
) -> bool {
    let reason = match state.chat_rooms_locked().await {
        Ok(false) => return false,
        Ok(true) => SYSTEM_LOCK_REASON,
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
    sink: &mut SplitSink<WebSocket, Message>,
) -> bool {
    match state.chat_rooms_locked().await {
        Ok(true) => {
            let _ = send_json(
                sink,
                &ChatMessage::System {
                    content: SYSTEM_LOCK_REASON.into(),
                    members: None,
                    participants: None,
                },
            )
            .await;
            let _ = sink.close().await;
            true
        }
        Ok(false) => false,
        Err(error) => {
            tracing::warn!("check system chat lock failed: {error}");
            false
        }
    }
}
