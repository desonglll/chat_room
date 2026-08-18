//! CLI client for room discovery, creation, and interactive chat.

use anyhow::{bail, Context, Result};
use clap::{Args, Parser, Subcommand};
use uuid::Uuid;

mod client_auth;
mod client_chat;
mod client_media;
mod client_render;

use client_auth::require_session;

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
            client_chat::join_room(
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
}
