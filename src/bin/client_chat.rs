//! WebSocket connection lifecycle for the terminal user interface.

use anyhow::{bail, Context, Result};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use uuid::Uuid;

use crate::client_chat_protocol::{
    command_frame, decode_server_message, emit_server_event, ServerMessage,
};
pub use crate::client_chat_protocol::{ChatCommand, ChatEvent, ChatMessage};

pub type ChatSender = mpsc::UnboundedSender<ChatCommand>;

pub struct ChatConnection {
    pub room_name: String,
    pub sender: ChatSender,
    pub events: mpsc::UnboundedReceiver<ChatEvent>,
}

pub async fn connect(
    http_base: &str,
    room_id: Uuid,
    token: Uuid,
    password: Option<&str>,
) -> Result<ChatConnection> {
    let websocket_base = if let Some(base) = http_base.strip_prefix("https://") {
        format!("wss://{base}")
    } else if let Some(base) = http_base.strip_prefix("http://") {
        format!("ws://{base}")
    } else {
        bail!("server URL must start with http:// or https://");
    };
    let (socket, _) = connect_async(format!(
        "{}/ws/{room_id}",
        websocket_base.trim_end_matches('/')
    ))
    .await
    .context("connect WebSocket")?;
    let (mut sink, mut stream) = socket.split();
    let greeting = match password {
        Some(password) => serde_json::json!({
            "type": "auth",
            "token": token,
            "password": password
        }),
        None => serde_json::json!({ "type": "join", "token": token }),
    };
    sink.send(Message::Text(greeting.to_string()))
        .await
        .context("send room authentication")?;

    let room_name = loop {
        match stream.next().await {
            Some(Ok(Message::Text(text))) => match decode_server_message(&text)? {
                ServerMessage::AuthOk { room_name } => break room_name,
                ServerMessage::AuthFail { reason } => bail!("join failed: {reason}"),
                _ => continue,
            },
            Some(Ok(Message::Ping(payload))) => {
                sink.send(Message::Pong(payload))
                    .await
                    .context("reply to WebSocket ping")?;
            }
            Some(Err(error)) => return Err(error).context("read authentication response"),
            Some(Ok(Message::Close(_))) | None => {
                bail!("server closed before authentication completed")
            }
            _ => {}
        }
    };

    let (command_tx, mut command_rx) = mpsc::unbounded_channel();
    let (event_tx, event_rx) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        loop {
            tokio::select! {
                command = command_rx.recv() => {
                    let Some(command) = command else { break };
                    if matches!(command, ChatCommand::Close) {
                        let _ = sink.send(Message::Close(None)).await;
                        break;
                    }
                    let frame = command_frame(command);
                    if let Err(error) = sink.send(Message::Text(frame.to_string())).await {
                        let _ = event_tx.send(ChatEvent::Error(format!("send failed: {error}")));
                        break;
                    }
                }
                frame = stream.next() => {
                    let Some(frame) = frame else { break };
                    match frame {
                        Ok(Message::Text(text)) => match decode_server_message(&text) {
                            Ok(message) => emit_server_event(&event_tx, message),
                            Err(error) => {
                                let _ = event_tx.send(ChatEvent::Error(error.to_string()));
                            }
                        },
                        Ok(Message::Ping(payload)) => {
                            if sink.send(Message::Pong(payload)).await.is_err() {
                                break;
                            }
                        }
                        Ok(Message::Close(_)) => break,
                        Err(error) => {
                            let _ = event_tx.send(ChatEvent::Error(format!("WebSocket error: {error}")));
                            break;
                        }
                        _ => {}
                    }
                }
            }
        }
        let _ = event_tx.send(ChatEvent::Closed);
    });

    Ok(ChatConnection {
        room_name,
        sender: command_tx,
        events: event_rx,
    })
}
