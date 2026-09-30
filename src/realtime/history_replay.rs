//! Persisted-history replay for a freshly authenticated WebSocket connection.
//!
//! Split out of `ws.rs` by TG-007 so the connection lifecycle and the replay concern stay
//! under the file-size gate separately. Behaviour is unchanged: read the three outbound
//! cursors, replay history up to the message boundary, then send `history_complete`.

use axum::extract::ws::{Message, WebSocket};
use futures_util::stream::SplitSink;
use uuid::Uuid;

use crate::models::ChatMessage;
use crate::realtime::outbound::OutboundCursors;
use crate::realtime::protocol::stored_message_to_chat;
use crate::state::SharedState;

use super::ws::send_json;

/// Replay persisted history to `sink` and return the cursors the outbound forwarder should
/// continue from. On failure the member is disconnected and `None` is returned; the caller
/// simply returns, exactly as the inlined code did before the split.
pub(super) async fn replay_history(
    state: &SharedState,
    room_id: Uuid,
    user_id: Uuid,
    sink: &mut SplitSink<WebSocket, Message>,
) -> Option<OutboundCursors> {
    let history_boundary = match state.latest_message_cursor(room_id).await {
        Ok(cursor) => cursor,
        Err(error) => {
            tracing::error!("read message history boundary failed: {}", error);
            let _ = send_json(
                sink,
                &ChatMessage::System {
                    content: "message history is temporarily unavailable".into(),
                    members: None,
                    participants: None,
                },
            )
            .await;
            state.member_disconnected(room_id, user_id).await;
            return None;
        }
    };
    let recall_boundary = match state.latest_recall_cursor(room_id).await {
        Ok(cursor) => cursor,
        Err(error) => {
            tracing::warn!("read recall boundary failed: {}", error);
            None
        }
    };
    let edit_boundary = match state.latest_edit_cursor(room_id).await {
        Ok(cursor) => cursor,
        Err(error) => {
            tracing::warn!("read edit boundary failed: {}", error);
            None
        }
    };

    let history = match state
        .message_history(
            room_id,
            state.realtime_config().history_replay_limit,
            history_boundary.as_ref(),
            Some(user_id),
        )
        .await
    {
        Ok(history) => history,
        Err(error) => {
            tracing::error!("load message history failed: {}", error);
            let _ = send_json(
                sink,
                &ChatMessage::System {
                    content: "message history is temporarily unavailable".into(),
                    members: None,
                    participants: None,
                },
            )
            .await;
            state.member_disconnected(room_id, user_id).await;
            return None;
        }
    };

    for message in history {
        if send_json(sink, &stored_message_to_chat(message))
            .await
            .is_err()
        {
            state.member_disconnected(room_id, user_id).await;
            return None;
        }
    }

    if send_json(sink, &ChatMessage::HistoryComplete)
        .await
        .is_err()
    {
        state.member_disconnected(room_id, user_id).await;
        return None;
    }

    Some(OutboundCursors {
        messages: history_boundary,
        recalls: recall_boundary,
        edits: edit_boundary,
    })
}
