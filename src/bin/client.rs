//! Echo Gate terminal client with a Ratatui default experience.

use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand};
use uuid::Uuid;

mod client_api;
mod client_api_features;
mod client_api_models;
mod client_auth;
mod client_chat;
mod client_chat_protocol;
mod client_media;
mod client_tui;

use client_api::ApiClient;
use client_auth::require_session;

#[derive(Parser)]
#[command(name = "chat-client", about = "Chat room CLI client")]
struct Cli {
    /// Server base URL.
    #[arg(long, default_value = "http://127.0.0.1:3000")]
    server: String,

    #[command(subcommand)]
    command: Option<Command>,
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
    let config = require_session()?;
    let api = ApiClient::new(http_base, config.token);
    let mut rooms = api.rooms().await?;
    rooms.extend(api.discover_rooms().await?);
    Ok(rooms
        .into_iter()
        .find(|room| room.name == name)
        .map(|room| room.id))
}

async fn list_rooms(http_base: &str) -> Result<()> {
    let config = require_session()?;
    let rooms = ApiClient::new(http_base, config.token).rooms().await?;
    if rooms.is_empty() {
        println!("No rooms. Create one with: client create --name <name>");
        return Ok(());
    }

    for room in rooms {
        let access = if room.has_password {
            "private"
        } else {
            "public"
        };
        println!("[{access}] {}  {}", room.name, room.id);
    }
    Ok(())
}

async fn create_room(http_base: &str, name: &str, password: Option<&str>) -> Result<()> {
    let config = require_session()?;
    let room = ApiClient::new(http_base, config.token)
        .create_room(name, password)
        .await?;
    let access = if room.has_password {
        "private"
    } else {
        "public"
    };
    println!("Created {access} room '{}' ({})", room.name, room.id);
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let http_base = cli.server.trim_end_matches('/');

    match cli.command {
        None => client_tui::run(http_base, None).await,
        Some(Command::Config) => client_auth::show_config(),
        Some(Command::Register { username, password }) => {
            client_auth::authenticate(http_base, "register", &username, &password).await
        }
        Some(Command::Login { username, password }) => {
            client_auth::authenticate(http_base, "login", &username, &password).await
        }
        Some(Command::Logout) => client_auth::logout(http_base).await,
        Some(Command::List) => list_rooms(http_base).await,
        Some(Command::Create { name, password }) => {
            create_room(http_base, &name, password.as_deref()).await
        }
        Some(Command::Join(arguments)) => {
            let room_id = match (arguments.room_id, arguments.room_name.as_deref()) {
                (Some(id), _) => id,
                (None, Some(name)) => lookup_room(http_base, name)
                    .await?
                    .with_context(|| format!("room '{name}' not found"))?,
                (None, None) => unreachable!("clap requires room name or id"),
            };
            let config = require_session()?;
            let membership = ApiClient::new(http_base, config.token)
                .join_room(room_id, arguments.password.as_deref())
                .await?;
            if membership.status != "active" {
                println!("Join request submitted; waiting for room approval.");
                return Ok(());
            }
            client_tui::run(http_base, Some((room_id, arguments.password))).await
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

    #[cfg(unix)]
    #[test]
    fn saved_session_is_owner_only() {
        use std::os::unix::fs::PermissionsExt;

        let directory =
            std::env::temp_dir().join(format!("chat-room-client-mode-{}", Uuid::new_v4()));
        let path = directory.join("config.json");
        client_auth::save_config_to(&path, &client_auth::UserConfig::default()).unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        let _ = std::fs::remove_dir_all(directory);
    }

    #[test]
    fn no_subcommand_selects_the_tui() {
        let cli = Cli::try_parse_from(["client"]).unwrap();
        assert!(cli.command.is_none());
    }
}
