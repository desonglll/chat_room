//! Interactive WebSocket chat session and slash-command handling.

use std::{path::Path, sync::Arc};

use anyhow::{bail, Context, Result};
use futures_util::{SinkExt, StreamExt};
use rustyline::{
    error::ReadlineError, Config as ReadlineConfig, DefaultEditor, EditMode, ExternalPrinter,
};
use serde::Deserialize;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use uuid::Uuid;

use crate::client_auth::history_path;
use crate::client_media::{self, Attachment, AttachmentIndex};
use crate::client_render;

#[derive(Deserialize)]
#[serde(tag = "type")]
enum ServerMessage {
    #[serde(rename = "auth_ok")]
    AuthOk { room_name: String },
    #[serde(rename = "auth_fail")]
    AuthFail { reason: String },
    #[serde(rename = "broadcast")]
    Broadcast {
        sender: String,
        content: String,
        #[serde(default)]
        attachment: Option<Attachment>,
        timestamp: String,
    },
    #[serde(rename = "system")]
    System { content: String },
}

pub async fn join_room(
    http_base: &str,
    room_id: Uuid,
    token: Uuid,
    username: &str,
    password: Option<&str>,
) -> Result<()> {
    let websocket_base = http_base
        .replace("http://", "ws://")
        .replace("https://", "wss://");
    let (socket, _) = connect_async(format!("{websocket_base}/ws/{room_id}"))
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

    let room_name = match stream.next().await {
        Some(Ok(Message::Text(text))) => match serde_json::from_str(&text)? {
            ServerMessage::AuthOk { room_name } => room_name,
            ServerMessage::AuthFail { reason } => bail!("join failed: {reason}"),
            _ => bail!("unexpected authentication response"),
        },
        Some(Err(error)) => return Err(error).context("read authentication response"),
        _ => bail!("server closed before authentication completed"),
    };

    println!("Joined '{room_name}' as '{username}'. Type /quit or press Ctrl-D to leave.");
    println!("Use /upload <path> to send a file and /download <attachment-id> [path] to save one.");
    println!("Emacs editing enabled: Ctrl-A, Ctrl-E, Ctrl-K, Ctrl-U and history.");

    let readline_config = ReadlineConfig::builder()
        .edit_mode(EditMode::Emacs)
        .auto_add_history(true)
        .build();
    let mut editor = DefaultEditor::with_config(readline_config)
        .context("initialize interactive line editor")?;
    let history = history_path();
    if history.exists() {
        let _ = editor.load_history(&history);
    }
    let mut printer = match editor.create_external_printer() {
        Ok(printer) => Some(printer),
        Err(error) => {
            eprintln!(
                "{}",
                client_render::system(&format!(
                    "asynchronous prompt refresh unavailable ({error}); using basic output"
                ))
            );
            None
        }
    };
    let current_user = username.to_string();
    let attachment_index: AttachmentIndex = Arc::default();
    let reader_attachments = attachment_index.clone();
    let reader_http_base = http_base.to_string();

    let reader = tokio::spawn(async move {
        let mut print_message = move |message: String| {
            if let Some(printer) = printer.as_mut() {
                let _ = printer.print(message);
            } else {
                println!("{message}");
            }
        };

        while let Some(frame) = stream.next().await {
            let message = match frame {
                Ok(Message::Text(text)) => match serde_json::from_str::<ServerMessage>(&text) {
                    Ok(message) => message,
                    Err(error) => {
                        print_message(client_render::error(&format!(
                            "invalid server message: {error}"
                        )));
                        continue;
                    }
                },
                Ok(Message::Close(_)) => {
                    print_message(client_render::system("connection closed"));
                    break;
                }
                Err(error) => {
                    print_message(client_render::error(&format!("WebSocket error: {error}")));
                    break;
                }
                _ => continue,
            };

            let rendered = match message {
                ServerMessage::Broadcast {
                    sender,
                    content,
                    attachment,
                    timestamp,
                } => {
                    if let Some(attachment) = attachment.as_ref() {
                        client_media::remember(&reader_attachments, attachment.clone());
                    }
                    client_render::chat(
                        &sender,
                        &content,
                        attachment.as_ref(),
                        &timestamp,
                        &current_user,
                        &reader_http_base,
                    )
                }
                ServerMessage::System { content } => client_render::system(&content),
                ServerMessage::AuthFail { reason } => client_render::error(&reason),
                ServerMessage::AuthOk { .. } => continue,
            };
            print_message(rendered);
        }
    });

    run_input_loop(
        &mut editor,
        &mut sink,
        &attachment_index,
        http_base,
        room_id,
        token,
        password,
    )
    .await;

    let _ = editor.save_history(&history);
    let _ = sink.send(Message::Close(None)).await;
    reader.abort();
    println!("Disconnected.");
    Ok(())
}

async fn run_input_loop<S>(
    editor: &mut DefaultEditor,
    sink: &mut S,
    attachments: &AttachmentIndex,
    http_base: &str,
    room_id: Uuid,
    token: Uuid,
    password: Option<&str>,
) where
    S: SinkExt<Message> + Unpin,
{
    loop {
        match editor.readline("> ") {
            Ok(line) => {
                let content = line.trim();
                if content == "/quit" {
                    break;
                }
                if content.is_empty() {
                    continue;
                }
                if handle_media_command(content, attachments, http_base, room_id, token, password)
                    .await
                {
                    continue;
                }
                let message = serde_json::json!({ "type": "message", "content": content });
                if sink.send(Message::Text(message.to_string())).await.is_err() {
                    break;
                }
            }
            Err(ReadlineError::Interrupted) => continue,
            Err(ReadlineError::Eof) => break,
            Err(error) => {
                eprintln!("{}", client_render::error(&format!("input error: {error}")));
                break;
            }
        }
    }
}

async fn handle_media_command(
    content: &str,
    attachments: &AttachmentIndex,
    http_base: &str,
    room_id: Uuid,
    token: Uuid,
    password: Option<&str>,
) -> bool {
    if content == "/upload" {
        eprintln!("{}", client_render::error("usage: /upload <path>"));
        return true;
    }
    if let Some(path) = content.strip_prefix("/upload ").map(str::trim) {
        match client_media::upload(http_base, room_id, token, password, Path::new(path)).await {
            Ok(attachment) => {
                client_media::remember(attachments, attachment.clone());
                println!(
                    "{}",
                    client_render::system(&format!("uploaded {}", attachment.file_name))
                );
            }
            Err(error) => eprintln!("{}", client_render::error(&error.to_string())),
        }
        return true;
    }
    if content == "/download" {
        eprintln!(
            "{}",
            client_render::error("usage: /download <attachment-id> [path]")
        );
        return true;
    }
    let Some(arguments) = content.strip_prefix("/download ").map(str::trim) else {
        return false;
    };
    let (id, destination) = arguments
        .split_once(' ')
        .map_or((arguments, None), |(id, path)| (id, Some(path.trim())));
    let result = async {
        let id = id.parse::<Uuid>().context("invalid attachment UUID")?;
        let attachment = client_media::find(attachments, id)
            .context("attachment is not in the current message history")?;
        let destination = destination.filter(|path| !path.is_empty()).map(Path::new);
        client_media::download(http_base, &attachment, destination).await
    }
    .await;
    match result {
        Ok(path) => println!(
            "{}",
            client_render::system(&format!("downloaded to {}", path.display()))
        ),
        Err(error) => eprintln!("{}", client_render::error(&error.to_string())),
    }
    true
}
