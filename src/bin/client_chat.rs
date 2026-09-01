//! WebSocket connection lifecycle for the terminal user interface.

use anyhow::{bail, Context, Result};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use uuid::Uuid;

use crate::client_chat_protocol::{
    command_frame, decode_server_message, emit_server_event, ServerMessage,
};
pub use crate::client_chat_protocol::{ChatCommand, ChatEvent, ChatMessage, DeliveryState};

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

#[cfg(test)]
mod tests {
    use futures_util::{SinkExt, StreamExt};
    use tokio::{net::TcpListener, time::Duration};
    use tokio_tungstenite::{accept_async, tungstenite::Message};

    use super::*;

    #[tokio::test]
    async fn send_command_reaches_the_socket_and_returns_as_an_event() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let room_id = Uuid::new_v4();
        let token = Uuid::new_v4();
        let client_message_id = Uuid::new_v4();
        let message_id = Uuid::new_v4();
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut socket = accept_async(stream).await.unwrap();
            let Message::Text(greeting) = socket.next().await.unwrap().unwrap() else {
                panic!("expected authentication frame");
            };
            let greeting: serde_json::Value = serde_json::from_str(&greeting).unwrap();
            assert_eq!(greeting["type"], "join");
            socket
                .send(Message::Text(
                    serde_json::json!({ "type": "auth_ok", "room_name": "Test room" }).to_string(),
                ))
                .await
                .unwrap();
            let Message::Text(frame) = socket.next().await.unwrap().unwrap() else {
                panic!("expected outgoing message frame");
            };
            let frame: serde_json::Value = serde_json::from_str(&frame).unwrap();
            assert_eq!(frame["type"], "message");
            assert_eq!(frame["content"], "hello socket");
            assert_eq!(frame["client_message_id"], client_message_id.to_string());
            socket
                .send(Message::Text(
                    serde_json::json!({
                        "type": "broadcast",
                        "message_id": message_id,
                        "client_message_id": client_message_id,
                        "sender": "alice",
                        "content": "hello socket",
                        "attachment": null,
                        "timestamp": "2026-08-31T12:00:00Z",
                        "recalled_at": null,
                        "edited_at": null
                    })
                    .to_string(),
                ))
                .await
                .unwrap();
        });

        let mut connection = connect(&format!("http://{address}"), room_id, token, None)
            .await
            .unwrap();
        connection
            .sender
            .send(ChatCommand::Send {
                content: "hello socket".into(),
                reply_to: None,
                client_message_id,
            })
            .unwrap();
        let event = tokio::time::timeout(Duration::from_secs(2), connection.events.recv())
            .await
            .expect("timed out waiting for echoed message")
            .expect("chat event channel closed");
        let ChatEvent::Message(message) = event else {
            panic!("expected echoed chat message");
        };
        assert_eq!(message.id, message_id);
        assert_eq!(message.client_message_id, Some(client_message_id));
        server.await.unwrap();
    }
}
