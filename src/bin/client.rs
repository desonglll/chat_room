//! CLI client for room discovery, creation, and interactive chat.

use anyhow::{bail, Context, Result};
use clap::{Args, Parser, Subcommand};
use colored::Colorize;
use futures_util::{SinkExt, StreamExt};
use rustyline::{
    error::ReadlineError, Config as ReadlineConfig, DefaultEditor, EditMode, ExternalPrinter,
};
use serde::Deserialize;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;
use uuid::Uuid;

mod client_auth;

use client_auth::{history_path, require_session};

#[derive(Parser)]
#[command(name = "chat-client", about = "Chat room CLI client")]
struct Cli {
    /// Server base URL.
    #[arg(long, default_value = "http://127.0.0.1:3000")]
    server: String,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Show the current saved login.
    Config,
    /// Register an account and save its login session.
    Register {
        #[arg(long)]
        username: String,
        #[arg(long)]
        password: String,
    },
    /// Log in and save the issued session.
    Login {
        #[arg(long)]
        username: String,
        #[arg(long)]
        password: String,
    },
    /// Revoke and clear the saved login session.
    Logout,
    /// List all rooms.
    List,
    /// Create a room. Omit --password for a public room.
    Create {
        #[arg(long)]
        name: String,
        #[arg(long)]
        password: Option<String>,
    },
    /// Join a room and start an interactive chat.
    Join(JoinArgs),
}

#[derive(Args)]
#[command(group = clap::ArgGroup::new("room").required(true).multiple(false))]
struct JoinArgs {
    /// Resolve and join a room by name.
    #[arg(long, group = "room")]
    room_name: Option<String>,

    /// Join directly by room UUID.
    #[arg(long, group = "room")]
    room_id: Option<Uuid>,

    /// Room password. Omit for public rooms.
    #[arg(long)]
    password: Option<String>,
}

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
        timestamp: String,
    },
    #[serde(rename = "system")]
    System { content: String },
}

async fn lookup_room(http_base: &str, name: &str) -> Result<Option<Uuid>> {
    let url = format!("{}/api/rooms?name={}", http_base, url_encode(name));
    let response = reqwest::get(&url).await.context("look up room by name")?;
    if !response.status().is_success() {
        bail!("room lookup returned {}", response.status());
    }
    let rooms: Vec<serde_json::Value> = response.json().await.context("decode room lookup")?;
    rooms
        .first()
        .and_then(|room| room["id"].as_str())
        .map(str::parse)
        .transpose()
        .context("server returned an invalid room UUID")
}

fn url_encode(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                output.push(byte as char);
            }
            b' ' => output.push_str("%20"),
            _ => output.push_str(&format!("%{:02X}", byte)),
        }
    }
    output
}

async fn list_rooms(http_base: &str) -> Result<()> {
    let url = format!("{}/api/rooms", http_base);
    let response = reqwest::get(&url).await.context("list rooms")?;
    if !response.status().is_success() {
        bail!("server returned {}", response.status());
    }

    let rooms: Vec<serde_json::Value> = response.json().await.context("decode room list")?;
    if rooms.is_empty() {
        println!("No rooms. Create one with: client create --name <name>");
        return Ok(());
    }

    for room in rooms {
        let access = if room["has_password"].as_bool().unwrap_or(false) {
            "private"
        } else {
            "public"
        };
        println!(
            "[{access}] {}  {}  {}",
            room["name"].as_str().unwrap_or("?"),
            room["id"].as_str().unwrap_or("?"),
            room["created_at"].as_str().unwrap_or("?")
        );
    }
    Ok(())
}

async fn create_room(http_base: &str, name: &str, password: Option<&str>) -> Result<()> {
    let response = reqwest::Client::new()
        .post(format!("{}/api/rooms", http_base))
        .json(&serde_json::json!({
            "name": name,
            "password": password.unwrap_or("")
        }))
        .send()
        .await
        .context("create room")?;

    match response.status().as_u16() {
        201 => {
            let room: serde_json::Value = response.json().await.context("decode new room")?;
            let access = if room["has_password"].as_bool().unwrap_or(false) {
                "private"
            } else {
                "public"
            };
            println!(
                "Created {access} room '{}' ({})",
                room["name"].as_str().unwrap_or(name),
                room["id"].as_str().unwrap_or("?")
            );
            Ok(())
        }
        409 => bail!("room name already exists"),
        status => bail!("server returned unexpected status {status}"),
    }
}

async fn join_room(
    http_base: &str,
    room_id: Uuid,
    token: Uuid,
    username: &str,
    password: Option<&str>,
) -> Result<()> {
    let websocket_base = http_base
        .replace("http://", "ws://")
        .replace("https://", "wss://");
    let url = format!("{}/ws/{}", websocket_base, room_id);
    let (socket, _) = connect_async(&url).await.context("connect WebSocket")?;
    let (mut sink, mut stream) = socket.split();

    let greeting = match password {
        Some(password) => serde_json::json!({
            "type": "auth",
            "token": token,
            "password": password
        }),
        None => serde_json::json!({
            "type": "join",
            "token": token
        }),
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

    println!(
        "Joined '{}' as '{}'. Type /quit or press Ctrl-D to leave.",
        room_name, username
    );
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
                render_system(&format!(
                    "asynchronous prompt refresh unavailable ({error}); using basic output"
                ))
            );
            None
        }
    };
    let current_user = username.to_string();

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
                        print_message(render_error(&format!("invalid server message: {error}")));
                        continue;
                    }
                },
                Ok(Message::Close(_)) => {
                    print_message(render_system("connection closed"));
                    break;
                }
                Err(error) => {
                    print_message(render_error(&format!("WebSocket error: {error}")));
                    break;
                }
                _ => continue,
            };

            let rendered = match message {
                ServerMessage::Broadcast {
                    sender,
                    content,
                    timestamp,
                } => render_chat(&sender, &content, &timestamp, &current_user),
                ServerMessage::System { content } => render_system(&content),
                ServerMessage::AuthFail { reason } => render_error(&reason),
                ServerMessage::AuthOk { .. } => continue,
            };
            print_message(rendered);
        }
    });

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
                let message = serde_json::json!({
                    "type": "message",
                    "content": content
                });
                if sink.send(Message::Text(message.to_string())).await.is_err() {
                    break;
                }
            }
            Err(ReadlineError::Interrupted) => continue,
            Err(ReadlineError::Eof) => break,
            Err(error) => {
                eprintln!("{}", render_error(&format!("input error: {error}")));
                break;
            }
        }
    }

    let _ = editor.save_history(&history);
    let _ = sink.send(Message::Close(None)).await;
    reader.abort();
    println!("Disconnected.");
    Ok(())
}

fn render_chat(sender: &str, content: &str, timestamp: &str, current_user: &str) -> String {
    let time = short_time(timestamp);
    let safe_sender = sanitize_terminal(sender);
    let safe_content = sanitize_terminal(content);
    if sender == current_user {
        format!(
            "{} {} {}",
            time.dimmed(),
            "[you]".green().bold(),
            safe_content
        )
    } else {
        format!(
            "{} {} {}",
            time.dimmed(),
            format!("[{safe_sender}]").blue().bold(),
            safe_content
        )
    }
}

fn render_system(content: &str) -> String {
    format!(
        "{} {}",
        "[system]".cyan().bold(),
        sanitize_terminal(content).dimmed()
    )
}

fn render_error(content: &str) -> String {
    format!("{} {}", "[error]".red().bold(), sanitize_terminal(content))
}

fn short_time(timestamp: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(timestamp)
        .map(|value| {
            value
                .with_timezone(&chrono::Local)
                .format("%H:%M:%S")
                .to_string()
        })
        .unwrap_or_else(|_| sanitize_terminal(timestamp))
}

fn sanitize_terminal(value: &str) -> String {
    value
        .chars()
        .filter(|character| !character.is_control())
        .collect()
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let http_base = cli.server.trim_end_matches('/');

    match cli.command {
        Command::Config => client_auth::show_config(),
        Command::Register { username, password } => {
            client_auth::authenticate(http_base, "register", &username, &password).await
        }
        Command::Login { username, password } => {
            client_auth::authenticate(http_base, "login", &username, &password).await
        }
        Command::Logout => client_auth::logout(http_base).await,
        Command::List => list_rooms(http_base).await,
        Command::Create { name, password } => {
            create_room(http_base, &name, password.as_deref()).await
        }
        Command::Join(arguments) => {
            let room_id = match (arguments.room_id, arguments.room_name.as_deref()) {
                (Some(id), _) => id,
                (None, Some(name)) => lookup_room(http_base, name)
                    .await?
                    .with_context(|| format!("room '{name}' not found"))?,
                (None, None) => unreachable!("clap requires room name or id"),
            };
            let config = require_session()?;
            join_room(
                http_base,
                room_id,
                config.token.expect("validated session token"),
                &config.username,
                arguments.password.as_deref(),
            )
            .await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_round_trip() {
        let directory =
            std::env::temp_dir().join(format!("chat-room-client-config-{}", Uuid::new_v4()));
        let path = directory.join("config.json");
        let expected = client_auth::UserConfig {
            username: "alice".to_string(),
            token: Some(Uuid::new_v4()),
        };

        client_auth::save_config_to(&path, &expected).unwrap();
        let actual = client_auth::load_config_from(&path).unwrap();

        assert_eq!(actual, expected);
        let _ = std::fs::remove_dir_all(directory);
    }

    #[test]
    fn chat_rendering_separates_message_types() {
        let own = render_chat("alice", "hello", "2026-08-14T06:00:00Z", "alice");
        let other = render_chat("bob", "hi", "2026-08-14T06:00:01Z", "alice");
        let system = render_system("bob joined");

        assert!(own.contains("[you]"));
        assert!(other.contains("[bob]"));
        assert!(system.contains("[system]"));
    }

    #[test]
    fn terminal_control_characters_are_removed() {
        assert_eq!(sanitize_terminal("hello\u{1b}[2J\nworld"), "hello[2Jworld");
    }
}
