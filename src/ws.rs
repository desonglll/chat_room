//! WebSocket authentication and room message forwarding.

use std::time::Duration;

use axum::{
    extract::ws::{Message, WebSocket},
    extract::{Path, State, WebSocketUpgrade},
    response::IntoResponse,
};
use futures_util::{SinkExt, StreamExt};
use sha2::{Digest, Sha256};
use tokio::{
    sync::broadcast,
    time::{interval, timeout, MissedTickBehavior},
};
use uuid::Uuid;

use crate::models::{ChatMessage, Room, StoredMessage, User};
use crate::state::{MessageCursor, RoomEvent, SharedState};

const AUTH_TIMEOUT: Duration = Duration::from_secs(10);
const HISTORY_REPLAY_LIMIT: i64 = 100;
const MESSAGE_POLL_INTERVAL: Duration = Duration::from_millis(250);
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(15);
const MESSAGE_POLL_LIMIT: i64 = 200;
const MAX_MESSAGE_CHARS: usize = 4096;
const MAX_PASSWORD_CHARS: usize = 256;

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

    if send_json(
        &mut sink,
        &ChatMessage::AuthOk {
            room_name: room.name.clone(),
        },
    )
    .await
    .is_err()
    {
        return;
    }

    let Some(mut room_messages) = state.subscribe(room_id).await else {
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
                },
            )
            .await;
            return;
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
                },
            )
            .await;
            return;
        }
    };

    for message in history {
        if send_json(&mut sink, &stored_message_to_chat(message))
            .await
            .is_err()
        {
            return;
        }
    }

    state
        .broadcast(
            room_id,
            ChatMessage::System {
                content: format!("{} joined the room", username),
            },
        )
        .await;

    let forwarding_state = state.clone();
    let forwarder = tokio::spawn(async move {
        let mut message_cursor = history_boundary;
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
                        let _ = send_json(&mut sink, &ChatMessage::System { content: reason }).await;
                        let _ = sink.close().await;
                        break;
                    }
                    Err(broadcast::error::RecvError::Lagged(count)) => {
                        tracing::warn!("client lagged by {} messages", count);
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                },
                _ = message_poll.tick() => {
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

        if let ChatMessage::Message { content } = message {
            let Some(content) = normalize_message(content) else {
                tracing::warn!("ignored invalid message from {}", username);
                continue;
            };
            match state
                .store_message(room_id, user.id, &username, &content)
                .await
            {
                Ok(_) => {}
                Err(error) => {
                    tracing::error!("persist chat message failed: {}", error);
                    state
                        .broadcast(
                            room_id,
                            ChatMessage::System {
                                content: format!(
                                    "message from {} was not saved or broadcast",
                                    username
                                ),
                            },
                        )
                        .await;
                }
            }
        }
    }

    forwarder.abort();
    state
        .broadcast(
            room_id,
            ChatMessage::System {
                content: format!("{} left the room", username),
            },
        )
        .await;
}

fn stored_message_to_chat(message: StoredMessage) -> ChatMessage {
    ChatMessage::Broadcast {
        message_id: message.id,
        sender_id: message.sender_id,
        sender: message.sender,
        content: message.content,
        timestamp: message.created_at,
    }
}

async fn authenticate(
    state: &SharedState,
    room: &Room,
    message: ChatMessage,
) -> Result<User, String> {
    let token = if room.has_password {
        match message {
            ChatMessage::Auth { token, password } => {
                if password.chars().count() > MAX_PASSWORD_CHARS {
                    return Err("password too long".into());
                }
                let mut hasher = Sha256::new();
                hasher.update(password.as_bytes());
                if hex::encode(hasher.finalize()) == room.password_hash {
                    token
                } else {
                    return Err("wrong password".into());
                }
            }
            ChatMessage::Join { .. } => {
                return Err("this room requires a password - send auth, not join".into());
            }
            _ => return Err("first message must be auth (room requires password)".into()),
        }
    } else {
        match message {
            ChatMessage::Join { token } | ChatMessage::Auth { token, .. } => token,
            _ => return Err("first message must be join or auth".into()),
        }
    };

    state
        .session_user(token)
        .await
        .map_err(|error| {
            tracing::error!("validate WebSocket session failed: {}", error);
            "authentication unavailable".to_string()
        })?
        .ok_or_else(|| "login required".to_string())
}

fn normalize_message(content: String) -> Option<String> {
    let content = content.trim().to_string();
    if content.is_empty() || content.chars().count() > MAX_MESSAGE_CHARS {
        return None;
    }
    Some(content)
}

async fn send_json(
    sink: &mut futures_util::stream::SplitSink<WebSocket, Message>,
    message: &ChatMessage,
) -> Result<(), axum::Error> {
    let json = serde_json::to_string(message).unwrap();
    sink.send(Message::Text(json)).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_and_limits_messages() {
        assert_eq!(normalize_message(" hello \n".into()).unwrap(), "hello");
        assert!(normalize_message(" \n".into()).is_none());
        assert!(normalize_message("x".repeat(MAX_MESSAGE_CHARS + 1)).is_none());
    }
}
