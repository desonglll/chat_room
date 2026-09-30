//! WebSocket authentication and chat message forwarding.

use std::time::Duration;

use axum::{
    extract::ws::{Message, WebSocket},
    extract::{Path, State, WebSocketUpgrade},
    response::IntoResponse,
};
use futures_util::{SinkExt, StreamExt};
use tokio::time::timeout;
use uuid::Uuid;

use crate::models::{ChatMessage, TypingAction, UserStatus};
use crate::realtime::history_replay::replay_history;
use crate::realtime::outbound::spawn_chat_forwarder;
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

    let chat = match state.chat(room_id).await {
        Some(chat) => chat,
        None => {
            let _ = send_json(
                &mut sink,
                &ChatMessage::AuthFail {
                    // Frozen wire value ("room", not "chat"): web/src/chatProtocol.ts AUTH_ERRORS.
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

    let authenticated = match authenticate(&state, &chat, first_message).await {
        Ok(authenticated) => authenticated,
        Err(reason) => {
            let _ = send_json(&mut sink, &ChatMessage::AuthFail { reason }).await;
            return;
        }
    };
    let user = authenticated.user;
    if reject_locked_auth(&state, room_id, &mut sink).await {
        return;
    }
    let username = user.username.clone();
    let membership = match state.membership_identity(room_id, user.id).await {
        Ok(Some((status, _))) if status == "active" => None,
        Ok(_) if chat.join_policy == "open" => {
            match state.request_chat_membership(room_id, user.id, true).await {
                Ok(membership) => Some(membership),
                Err(error) => {
                    tracing::error!("activate open chat membership failed: {}", error);
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
            tracing::error!("load chat membership failed: {}", error);
            return;
        }
    };
    let (members, first_connection) = state.member_connected(room_id, &user).await;
    touch_last_seen(&state, user.id).await;
    let participants = match state.chat_participants(room_id).await {
        Ok(participants) => participants,
        Err(error) => {
            tracing::error!("record chat participant failed: {}", error);
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
    let read_receipts = match state.chat_read_receipts(room_id).await {
        Ok(receipts) => receipts,
        Err(error) => {
            tracing::warn!("load chat read receipts failed: {}", error);
            Vec::new()
        }
    };

    let display_room_name = match state.conversation_summary(user.id, room_id).await {
        Ok(Some(conversation)) => conversation.title,
        Ok(None) => chat.title.clone(),
        Err(error) => {
            tracing::warn!("load viewer chat title failed: {error}");
            chat.title.clone()
        }
    };
    // TG-505: statuses and the connected list are filtered by last-seen privacy per viewer.
    let (statuses, visible_members) = state
        .presence_snapshot(
            user.id,
            room_id,
            &members,
            &participants,
            chrono::Utc::now(),
        )
        .await;
    if send_json(
        &mut sink,
        &ChatMessage::AuthOk {
            room_name: display_room_name,
            statuses,
            members: visible_members,
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

    let Some(chat_messages) = state.subscribe(room_id).await else {
        state.member_disconnected(room_id, user.id).await;
        return;
    };

    let Some(cursors) = replay_history(&state, room_id, user.id, &mut sink).await else {
        return;
    };

    if membership.is_some() {
        state
            .broadcast(
                room_id,
                ChatMessage::System {
                    // Frozen wire value: web/src/chatProtocol.ts `/^(.*) joined the room$/`.
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
    if first_connection || membership.is_some() {
        // After the join system/presence frame; clients must not rely on that order.
        state
            .broadcast(
                room_id,
                ChatMessage::UserStatusChanged {
                    user_id: user.id,
                    status: UserStatus::Online,
                },
            )
            .await;
    }

    let forwarder = spawn_chat_forwarder(
        state.clone(),
        room_id,
        user.id,
        authenticated.session_id,
        sink,
        chat_messages,
        cursors,
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
    let last_seen = touch_last_seen(&state, user.id).await;
    if last_connection {
        state
            .broadcast(
                room_id,
                ChatMessage::Typing {
                    content: String::new(),
                    action: TypingAction::Cancel,
                    user_id: Some(user.id),
                    username: Some(username.clone()),
                },
            )
            .await;
        // Before the trailing presence frame; clients must not rely on that order.
        state
            .broadcast(
                room_id,
                ChatMessage::UserStatusChanged {
                    user_id: user.id,
                    status: UserStatus::Offline { last_seen },
                },
            )
            .await;
        let participants = state.chat_participants(room_id).await.unwrap_or_default();
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

/// Persist activity now (TG-505) and return the instant recorded. A storage failure only
/// costs last-seen accuracy, so it is logged and never fails the socket.
async fn touch_last_seen(state: &SharedState, user_id: Uuid) -> chrono::DateTime<chrono::Utc> {
    let now = chrono::Utc::now();
    if let Err(error) = state.touch_last_seen(user_id, now).await {
        tracing::warn!("persist last-seen failed: {error}");
    }
    now
}

pub(super) async fn send_json(
    sink: &mut futures_util::stream::SplitSink<WebSocket, Message>,
    message: &ChatMessage,
) -> Result<(), axum::Error> {
    let json = serde_json::to_string(message).unwrap();
    sink.send(Message::Text(json)).await
}
