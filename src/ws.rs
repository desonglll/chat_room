//! WebSocket authentication and room message forwarding.

use std::time::Duration;

use axum::{
    extract::ws::{Message, WebSocket},
    extract::{Path, State, WebSocketUpgrade},
    response::IntoResponse,
};
use futures_util::{SinkExt, StreamExt};
use tokio::{
    sync::broadcast,
    time::{interval, timeout, MissedTickBehavior},
};
use uuid::Uuid;

use crate::message_store::MessageCursor;
use crate::models::{ChatMessage, StoredMessage};
use crate::state::{RoomEvent, SharedState};
use crate::ws_auth::authenticate;
use crate::ws_inbound::handle_client_message;

const AUTH_TIMEOUT: Duration = Duration::from_secs(10);
const HISTORY_REPLAY_LIMIT: i64 = 100;
const MESSAGE_POLL_INTERVAL: Duration = Duration::from_millis(250);
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(15);
const MESSAGE_POLL_LIMIT: i64 = 200;

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Path(room_id): Path<Uuid>,
    State(state): State<SharedState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, room_id, state))
}

async fn handle_socket(socket: WebSocket, room_id: Uuid, state: SharedState) {
    let (mut sink, mut stream) = socket.split();

    let room = match state.room(room_id).await {
        Some(room) => room,
        None => {
            let _ = send_json(
                &mut sink,
                &ChatMessage::AuthFail {
                    reason: "room not found".into(),
                },
            )
            .await;
            return;
        }
    };

    let first_raw = match timeout(AUTH_TIMEOUT, stream.next()).await {
        Ok(Some(Ok(Message::Text(text)))) => text.to_string(),
        Err(_) => {
            let _ = send_json(
                &mut sink,
                &ChatMessage::AuthFail {
                    reason: "authentication timeout".into(),
                },
            )
            .await;
            return;
        }
        _ => return,
    };

    let first_message: ChatMessage = match serde_json::from_str(&first_raw) {
        Ok(message) => message,
        Err(_) => {
            let _ = send_json(
                &mut sink,
                &ChatMessage::AuthFail {
                    reason: "invalid json".into(),
                },
            )
            .await;
            return;
        }
    };

    let user = match authenticate(&state, &room, first_message).await {
        Ok(user) => user,
        Err(reason) => {
            let _ = send_json(&mut sink, &ChatMessage::AuthFail { reason }).await;
            return;
        }
    };
    let username = user.username.clone();
    let membership = match state.membership_identity(room_id, user.id).await {
        Ok(Some((status, _))) if status == "active" => None,
        Ok(_) if room.join_policy == "open" => {
            match state.request_room_membership(room_id, user.id, true).await {
                Ok(membership) => Some(membership),
                Err(error) => {
                    tracing::error!("activate open room membership failed: {}", error);
                    let _ = send_json(
                        &mut sink,
                        &ChatMessage::AuthFail {
                            reason: "authentication unavailable".into(),
                        },
                    )
                    .await;
                    return;
                }
            }
        }
        Ok(Some((status, _))) if status == "pending" => {
            let _ = send_json(
                &mut sink,
                &ChatMessage::AuthFail {
                    reason: "membership pending".into(),
                },
            )
            .await;
            return;
        }
        Ok(_) => {
            let _ = send_json(
                &mut sink,
                &ChatMessage::AuthFail {
                    reason: "membership required".into(),
                },
            )
            .await;
            return;
        }
        Err(error) => {
            tracing::error!("load room membership failed: {}", error);
            return;
        }
    };
    let (members, first_connection) = state.member_connected(room_id, &user).await;
    let participants = match state.room_participants(room_id).await {
        Ok(participants) => participants,
        Err(error) => {
            tracing::error!("record room participant failed: {}", error);
            state.member_disconnected(room_id, user.id).await;
            let _ = send_json(
                &mut sink,
                &ChatMessage::AuthFail {
                    reason: "authentication unavailable".into(),
                },
            )
            .await;
            return;
        }
    };
    let read_receipts = match state.room_read_receipts(room_id).await {
        Ok(receipts) => receipts,
        Err(error) => {
            tracing::warn!("load room read receipts failed: {}", error);
            Vec::new()
        }
    };

    if send_json(
        &mut sink,
        &ChatMessage::AuthOk {
            room_name: room.name.clone(),
            members: members.clone(),
            participants: participants.clone(),
            read_receipts,
        },
    )
    .await
    .is_err()
    {
        state.member_disconnected(room_id, user.id).await;
        return;
    }

    let Some(mut room_messages) = state.subscribe(room_id).await else {
        state.member_disconnected(room_id, user.id).await;
        return;
    };

    let history_boundary = match state.latest_message_cursor(room_id).await {
        Ok(cursor) => cursor,
        Err(error) => {
            tracing::error!("read message history boundary failed: {}", error);
            let _ = send_json(
                &mut sink,
                &ChatMessage::System {
                    content: "message history is temporarily unavailable".into(),
                    members: None,
                    participants: None,
                },
            )
            .await;
            state.member_disconnected(room_id, user.id).await;
            return;
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
        .message_history(room_id, HISTORY_REPLAY_LIMIT, history_boundary.as_ref())
        .await
    {
        Ok(history) => history,
        Err(error) => {
            tracing::error!("load message history failed: {}", error);
            let _ = send_json(
                &mut sink,
                &ChatMessage::System {
                    content: "message history is temporarily unavailable".into(),
                    members: None,
                    participants: None,
                },
            )
            .await;
            state.member_disconnected(room_id, user.id).await;
            return;
        }
    };

    for message in history {
        if send_json(&mut sink, &stored_message_to_chat(message))
            .await
            .is_err()
        {
            state.member_disconnected(room_id, user.id).await;
            return;
        }
    }

    if membership.is_some() {
        state
            .broadcast(
                room_id,
                ChatMessage::System {
                    content: format!("{} joined the room", username),
                    members: Some(members.clone()),
                    participants: Some(participants.clone()),
                },
            )
            .await;
    } else if first_connection {
        state
            .broadcast(
                room_id,
                ChatMessage::Presence {
                    members: members.clone(),
                    participants: participants.clone(),
                },
            )
            .await;
    }

    let forwarding_state = state.clone();
    let forwarding_user_id = user.id;
    let forwarder = tokio::spawn(async move {
        let mut message_cursor = history_boundary;
        let mut recall_cursor = recall_boundary;
        let mut edit_cursor = edit_boundary;
        let mut message_poll = interval(MESSAGE_POLL_INTERVAL);
        let mut heartbeat = interval(HEARTBEAT_INTERVAL);
        message_poll.set_missed_tick_behavior(MissedTickBehavior::Delay);
        heartbeat.set_missed_tick_behavior(MissedTickBehavior::Delay);
        heartbeat.tick().await;

        loop {
            tokio::select! {
                event = room_messages.recv() => match event {
                    Ok(RoomEvent::Message(message)) => {
                        if send_json(&mut sink, &message).await.is_err() {
                            break;
                        }
                    }
                    Ok(RoomEvent::Disconnect { reason }) => {
                        let _ = send_json(
                            &mut sink,
                            &ChatMessage::System {
                                content: reason,
                                members: None,
                                participants: None,
                            },
                        )
                        .await;
                        let _ = sink.close().await;
                        break;
                    }
                    Ok(RoomEvent::DisconnectUser { user_id, reason }) => {
                        if user_id == forwarding_user_id {
                            let _ = send_json(
                                &mut sink,
                                &ChatMessage::System {
                                    content: reason,
                                    members: None,
                                    participants: None,
                                },
                            )
                            .await;
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
                    match forwarding_state.is_room_participant(room_id, forwarding_user_id).await {
                        Ok(true) => {}
                        Ok(false) => {
                            let _ = send_json(
                                &mut sink,
                                &ChatMessage::System {
                                    content: "membership left".into(),
                                    members: None,
                                    participants: None,
                                },
                            ).await;
                            let _ = sink.close().await;
                            return;
                        }
                        Err(error) => tracing::warn!("check room membership failed: {}", error),
                    }
                    match forwarding_state
                        .messages_after(room_id, message_cursor.as_ref(), MESSAGE_POLL_LIMIT)
                        .await
                    {
                        Ok(messages) => {
                            for message in messages {
                                message_cursor = Some(MessageCursor {
                                    created_at: message.created_at,
                                    id: message.id,
                                });
                                if send_json(&mut sink, &stored_message_to_chat(message)).await.is_err() {
                                    return;
                                }
                            }
                        }
                        Err(error) => {
                            tracing::warn!("poll room messages failed: {}", error);
                        }
                    }
                    match forwarding_state
                        .recalls_after(room_id, recall_cursor.as_ref(), MESSAGE_POLL_LIMIT)
                        .await
                    {
                        Ok(recalls) => {
                            for recalled in recalls {
                                recall_cursor = Some(recalled.clone());
                                if send_json(
                                    &mut sink,
                                    &ChatMessage::MessageRecalled {
                                        message_id: recalled.id,
                                        recalled_at: recalled.recalled_at,
                                    },
                                )
                                .await
                                .is_err()
                                {
                                    return;
                                }
                            }
                        }
                        Err(error) => tracing::warn!("poll recalled messages failed: {}", error),
                    }
                    match forwarding_state
                        .edits_after(room_id, edit_cursor.as_ref(), MESSAGE_POLL_LIMIT)
                        .await
                    {
                        Ok(edits) => {
                            for edited in edits {
                                edit_cursor = Some(edited.clone());
                                if send_json(
                                    &mut sink,
                                    &ChatMessage::MessageEdited {
                                        message_id: edited.id,
                                        content: edited.content,
                                        edited_at: edited.edited_at,
                                    },
                                )
                                .await
                                .is_err()
                                {
                                    return;
                                }
                            }
                        }
                        Err(error) => tracing::warn!("poll edited messages failed: {}", error),
                    }
                },
                _ = heartbeat.tick() => {
                    if sink.send(Message::Ping(Vec::new())).await.is_err() {
                        break;
                    }
                },
            }
        }
    });

    while let Some(frame) = stream.next().await {
        let text = match frame {
            Ok(Message::Text(text)) => text.to_string(),
            Ok(Message::Close(_)) | Err(_) => break,
            _ => continue,
        };

        let message: ChatMessage = match serde_json::from_str(&text) {
            Ok(message) => message,
            Err(_) => continue,
        };

        handle_client_message(&state, room_id, &user, message).await;
    }

    forwarder.abort();
    let (members, last_connection) = state.member_disconnected(room_id, user.id).await;
    if last_connection {
        state
            .broadcast(
                room_id,
                ChatMessage::Typing {
                    content: String::new(),
                    user_id: Some(user.id),
                    username: Some(username.clone()),
                },
            )
            .await;
        let participants = state.room_participants(room_id).await.unwrap_or_default();
        state
            .broadcast(
                room_id,
                ChatMessage::Presence {
                    members,
                    participants,
                },
            )
            .await;
    }
}

fn stored_message_to_chat(message: StoredMessage) -> ChatMessage {
    ChatMessage::Broadcast {
        message_id: message.id,
        sender_id: message.sender_id,
        sender: message.sender,
        sender_avatar: message.sender_avatar,
        content: message.content,
        attachment: message.attachment,
        reply_to: message.reply_to,
        recalled_at: message.recalled_at,
        edited_at: message.edited_at,
        timestamp: message.created_at,
    }
}

async fn send_json(
    sink: &mut futures_util::stream::SplitSink<WebSocket, Message>,
    message: &ChatMessage,
) -> Result<(), axum::Error> {
    let json = serde_json::to_string(message).unwrap();
    sink.send(Message::Text(json)).await
}
