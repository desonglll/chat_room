//! WebSocket authentication and room message forwarding.

use std::time::Duration;

use axum::{
    extract::ws::{Message, WebSocket},
    extract::{Path, State, WebSocketUpgrade},
    response::IntoResponse,
};
use futures_util::{SinkExt, StreamExt};
use tokio::time::timeout;
use uuid::Uuid;

use crate::models::ChatMessage;
use crate::realtime::outbound::{spawn_room_forwarder, OutboundCursors};
use crate::realtime::protocol::stored_message_to_chat;
use crate::realtime::system_lock::reject_locked_auth;
use crate::state::SharedState;
use crate::ws_auth::authenticate;
use crate::ws_inbound::handle_client_message;

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

    let auth_timeout = Duration::from_secs(state.realtime_config().auth_timeout_secs);
    let first_raw = match timeout(auth_timeout, stream.next()).await {
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
    if reject_locked_auth(&state, room_id, &mut sink).await {
        return;
    }
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

    let display_room_name = match state.conversation_summary(user.id, room_id).await {
        Ok(Some(conversation)) => conversation.title,
        Ok(None) => room.name.clone(),
        Err(error) => {
            tracing::warn!("load viewer room title failed: {error}");
            room.name.clone()
        }
    };
    if send_json(
        &mut sink,
        &ChatMessage::AuthOk {
            room_name: display_room_name,
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

    let Some(room_messages) = state.subscribe(room_id).await else {
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
        .message_history(
            room_id,
            state.realtime_config().history_replay_limit,
            history_boundary.as_ref(),
            Some(user.id),
        )
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

    if send_json(&mut sink, &ChatMessage::HistoryComplete)
        .await
        .is_err()
    {
        state.member_disconnected(room_id, user.id).await;
        return;
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

    let forwarder = spawn_room_forwarder(
        state.clone(),
        room_id,
        user.id,
        sink,
        room_messages,
        OutboundCursors {
            messages: history_boundary,
            recalls: recall_boundary,
            edits: edit_boundary,
        },
    );

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

pub(super) async fn send_json(
    sink: &mut futures_util::stream::SplitSink<WebSocket, Message>,
    message: &ChatMessage,
) -> Result<(), axum::Error> {
    let json = serde_json::to_string(message).unwrap();
    sink.send(Message::Text(json)).await
}
