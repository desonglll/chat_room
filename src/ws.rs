//! WebSocket authentication and room message forwarding.

use axum::{
    extract::ws::{Message, WebSocket},
    extract::{Path, State, WebSocketUpgrade},
    response::IntoResponse,
};
use futures_util::{SinkExt, StreamExt};
use sha2::{Digest, Sha256};
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::models::{ChatMessage, Room};
use crate::state::SharedState;

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

    let first_raw = match stream.next().await {
        Some(Ok(Message::Text(text))) => text.to_string(),
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

    let username = match authenticate(&room, first_message) {
        Ok(username) => username,
        Err(reason) => {
            let _ = send_json(&mut sink, &ChatMessage::AuthFail { reason }).await;
            return;
        }
    };

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

    state
        .broadcast(
            room_id,
            ChatMessage::System {
                content: format!("{} joined the room", username),
            },
        )
        .await;

    let forwarder = tokio::spawn(async move {
        loop {
            match room_messages.recv().await {
                Ok(message) => {
                    if send_json(&mut sink, &message).await.is_err() {
                        break;
                    }
                }
                Err(broadcast::error::RecvError::Lagged(count)) => {
                    tracing::warn!("client lagged by {} messages", count);
                }
                Err(broadcast::error::RecvError::Closed) => break,
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
            state
                .broadcast(
                    room_id,
                    ChatMessage::Broadcast {
                        sender: username.clone(),
                        content,
                        timestamp: chrono::Utc::now(),
                    },
                )
                .await;
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

fn authenticate(room: &Room, message: ChatMessage) -> Result<String, String> {
    if room.has_password {
        match message {
            ChatMessage::Auth { username, password } => {
                let mut hasher = Sha256::new();
                hasher.update(password.as_bytes());
                if hex::encode(hasher.finalize()) == room.password_hash {
                    Ok(username)
                } else {
                    Err("wrong password".into())
                }
            }
            ChatMessage::Join { .. } => {
                Err("this room requires a password - send auth, not join".into())
            }
            _ => Err("first message must be auth (room requires password)".into()),
        }
    } else {
        match message {
            ChatMessage::Join { username } | ChatMessage::Auth { username, .. } => Ok(username),
            _ => Err("first message must be join or auth".into()),
        }
    }
}

async fn send_json(
    sink: &mut futures_util::stream::SplitSink<WebSocket, Message>,
    message: &ChatMessage,
) -> Result<(), axum::Error> {
    let json = serde_json::to_string(message).unwrap();
    sink.send(Message::Text(json)).await
}
