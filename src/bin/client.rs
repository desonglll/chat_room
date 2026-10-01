//! Echo Gate terminal client with a Ratatui default experience.

use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand};
use uuid::Uuid;

mod client_api;
mod client_api_features;
mod client_api_messages;
mod client_api_models;
mod client_api_polls_folders;
mod client_api_social;
mod client_auth;
mod client_chat;
mod client_chat_extras;
mod client_chat_media;
mod client_chat_protocol;
mod client_media;
#[cfg(test)]
mod client_messages_e2e_tests;
#[cfg(test)]
mod client_social_e2e_tests;
mod client_tui;

use client_api::ApiClient;
use client_auth::require_session;

#[derive(Parser)]
#[command(name = "chat-client", about = "Echo Gate chat CLI client")]
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
    /// List all chats.
    List,
    /// Create a chat. Omit --password for a public chat.
    Create {
        #[arg(long)]
        name: String,
        #[arg(long)]
        password: Option<String>,
    },
    /// Join a chat and start an interactive chat.
    Join(JoinArgs),
}

#[derive(Args)]
#[command(group = clap::ArgGroup::new("chat").required(true).multiple(false))]
struct JoinArgs {
    /// Resolve and join a chat by name.
    #[arg(long, group = "chat")]
    room_name: Option<String>,

    /// Join directly by chat UUID.
    #[arg(long, group = "chat")]
    room_id: Option<Uuid>,

    /// Chat password. Omit for public chats.
    #[arg(long)]
    password: Option<String>,
}

async fn lookup_chat(http_base: &str, name: &str) -> Result<Option<Uuid>> {
    let config = require_session()?;
    let api = ApiClient::new(http_base, config.token);
    let mut chats = api.chats().await?;
    chats.extend(api.discover_chats().await?);
    Ok(chats
        .into_iter()
        .find(|chat| chat.title == name)
        .map(|chat| chat.id))
}

async fn list_chats(http_base: &str) -> Result<()> {
    let config = require_session()?;
    let chats = ApiClient::new(http_base, config.token).chats().await?;
    if chats.is_empty() {
        println!("No chats. Create one with: client create --name <name>");
        return Ok(());
    }

    for chat in chats {
        let access = if chat.has_password {
            "private"
        } else {
            "public"
        };
        println!("[{access}] {}  {}", chat.title, chat.id);
    }
    Ok(())
}

async fn create_chat(http_base: &str, name: &str, password: Option<&str>) -> Result<()> {
    let config = require_session()?;
    let chat = ApiClient::new(http_base, config.token)
        .create_chat(name, password)
        .await?;
    let access = if chat.has_password {
        "private"
    } else {
        "public"
    };
    println!("Created {access} chat '{}' ({})", chat.title, chat.id);
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
        Some(Command::List) => list_chats(http_base).await,
        Some(Command::Create { name, password }) => {
            create_chat(http_base, &name, password.as_deref()).await
        }
        Some(Command::Join(arguments)) => {
            let room_id = match (arguments.room_id, arguments.room_name.as_deref()) {
                (Some(id), _) => id,
                (None, Some(name)) => lookup_chat(http_base, name)
                    .await?
                    .with_context(|| format!("chat '{name}' not found"))?,
                (None, None) => unreachable!("clap requires chat name or id"),
            };
            let config = require_session()?;
            let membership = ApiClient::new(http_base, config.token)
                .join_chat(room_id, arguments.password.as_deref())
                .await?;
            if membership.status != "active" {
                println!("Join request submitted; waiting for chat approval.");
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
