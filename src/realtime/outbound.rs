//! Outbound WebSocket events, database polling, and heartbeat delivery.

use std::time::Duration;

use axum::extract::ws::{Message, WebSocket};
use futures_util::{stream::SplitSink, SinkExt};
use tokio::{
    sync::broadcast,
    task::JoinHandle,
    time::{interval, MissedTickBehavior},
};
use uuid::Uuid;

use crate::message_store::MessageCursor;
use crate::messages::actions::{EditCursor, RecallCursor};
use crate::models::ChatMessage;
use crate::realtime::protocol::{advance_message_cursor, stored_message_to_chat};
use crate::realtime::system_lock::close_if_locked;
use crate::state::{RoomEvent, SharedState};

use super::ws::send_json;

pub(super) struct OutboundCursors {
    pub messages: Option<MessageCursor>,
    pub recalls: Option<RecallCursor>,
    pub edits: Option<EditCursor>,
}

pub(super) fn spawn_room_forwarder(
    state: SharedState,
    room_id: Uuid,
    user_id: Uuid,
    session_id: Uuid,
    mut sink: SplitSink<WebSocket, Message>,
    mut room_messages: broadcast::Receiver<RoomEvent>,
    mut cursors: OutboundCursors,
) -> JoinHandle<()> {
    let poll_interval = Duration::from_millis(state.realtime_config().poll_interval_ms);
    let heartbeat_interval = Duration::from_secs(state.realtime_config().heartbeat_interval_secs);
    let message_poll_limit = state.realtime_config().message_poll_limit;
    tokio::spawn(async move {
        let mut message_poll = interval(poll_interval);
        let mut heartbeat = interval(heartbeat_interval);
        message_poll.set_missed_tick_behavior(MissedTickBehavior::Delay);
        heartbeat.set_missed_tick_behavior(MissedTickBehavior::Delay);
        heartbeat.tick().await;

        loop {
            tokio::select! {
                event = room_messages.recv() => match event {
                    Ok(RoomEvent::Message(message)) => {
                        advance_message_cursor(&mut cursors.messages, &message);
                        if send_json(&mut sink, &message).await.is_err() {
                            break;
                        }
                    }
                    Ok(RoomEvent::Disconnect { reason }) => {
                        let _ = send_json(
                            &mut sink,
                            &ChatMessage::System { content: reason, members: None, participants: None },
                        ).await;
                        let _ = sink.close().await;
                        break;
                    }
                    Ok(RoomEvent::DisconnectUser { user_id: target_id, reason }) => {
                        if target_id == user_id {
                            let _ = send_json(
                                &mut sink,
                                &ChatMessage::System { content: reason, members: None, participants: None },
                            ).await;
                            let _ = sink.close().await;
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(count)) => {
                        tracing::warn!("client lagged by {} messages", count);
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                },
                _ = message_poll.tick() => {
                    if !poll_database_updates(
                        &state,
                        room_id,
                        user_id,
                        session_id,
                        message_poll_limit,
                        &mut cursors,
                        &mut sink,
                    ).await {
                        return;
                    }
                },
                _ = heartbeat.tick() => {
                    if close_if_locked(&state, room_id, &mut sink).await {
                        return;
                    }
                    if sink.send(Message::Ping(Vec::new())).await.is_err() {
                        break;
                    }
                },
            }
        }
    })
}

async fn poll_database_updates(
    state: &SharedState,
    room_id: Uuid,
    user_id: Uuid,
    session_id: Uuid,
    limit: i64,
    cursors: &mut OutboundCursors,
    sink: &mut SplitSink<WebSocket, Message>,
) -> bool {
    match state.session_active(session_id).await {
        Ok(true) => {}
        Ok(false) => {
            let _ = send_json(
                sink,
                &ChatMessage::System {
                    content: "session revoked".into(),
                    members: None,
                    participants: None,
                },
            )
            .await;
            let _ = sink.close().await;
            return false;
        }
        Err(error) => tracing::warn!("validate live room session failed: {error}"),
    }

    match state.is_room_participant(room_id, user_id).await {
        Ok(true) => {}
        Ok(false) => {
            let _ = send_json(
                sink,
                &ChatMessage::System {
                    content: "membership left".into(),
                    members: None,
                    participants: None,
                },
            )
            .await;
            let _ = sink.close().await;
            return false;
        }
        Err(error) => tracing::warn!("check room membership failed: {}", error),
    }

    match state
        .messages_after(room_id, cursors.messages.as_ref(), limit, Some(user_id))
        .await
    {
        Ok(messages) => {
            for message in messages {
                cursors.messages = Some(MessageCursor {
                    created_at: message.created_at,
                    id: message.id,
                });
                if send_json(sink, &stored_message_to_chat(message))
                    .await
                    .is_err()
                {
                    return false;
                }
            }
        }
        Err(error) => tracing::warn!("poll room messages failed: {}", error),
    }
    match state
        .recalls_after(room_id, cursors.recalls.as_ref(), limit)
        .await
    {
        Ok(recalls) => {
            for recalled in recalls {
                cursors.recalls = Some(recalled.clone());
                if send_json(
                    sink,
                    &ChatMessage::MessageRecalled {
                        message_id: recalled.id,
                        recalled_at: recalled.recalled_at,
                    },
                )
                .await
                .is_err()
                {
                    return false;
                }
            }
        }
        Err(error) => tracing::warn!("poll recalled messages failed: {}", error),
    }
    match state
        .edits_after(room_id, cursors.edits.as_ref(), limit)
        .await
    {
        Ok(edits) => {
            for edited in edits {
                cursors.edits = Some(edited.clone());
                if send_json(
                    sink,
                    &ChatMessage::MessageEdited {
                        message_id: edited.id,
                        content: edited.content,
                        edited_at: edited.edited_at,
                    },
                )
                .await
                .is_err()
                {
                    return false;
                }
            }
        }
        Err(error) => tracing::warn!("poll edited messages failed: {}", error),
    }
    true
}
