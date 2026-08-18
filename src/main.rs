//! Server binary with optional embedded web client.

use std::{net::SocketAddr, path::PathBuf, sync::Arc};

use anyhow::{Context, Result};
use chat_room::{build_app_with_web, config::AppConfig, state::AppState};
use clap::Parser;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Debug, Parser)]
#[command(name = "server", about = "SQLite-backed chat room server")]
struct Args {
    /// Compatibility flag; the browser client is enabled by default.
    #[arg(long)]
    web: bool,

    /// Run the API and WebSocket server without the browser client.
    #[arg(long)]
    no_web: bool,

    /// Address and port to listen on.
    #[arg(long, default_value = "0.0.0.0:3000")]
    listen: SocketAddr,

    /// Override the port from --listen.
    #[arg(short = 'p', long, value_name = "PORT")]
    port: Option<u16>,

    /// SQLite database path. Overrides CHAT_ROOM_DATABASE_PATH.
    #[arg(long, value_name = "PATH")]
    database: Option<PathBuf>,

    /// TOML configuration path. A missing file uses built-in defaults.
    #[arg(long, default_value = "chat-room.toml", value_name = "PATH")]
    config: PathBuf,
}

impl Args {
    fn listen_addr(&self) -> SocketAddr {
        match self.port {
            Some(port) => SocketAddr::new(self.listen.ip(), port),
            None => self.listen,
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "chat_room=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let args = Args::parse();
    let listen_addr = args.listen_addr();
    let config = AppConfig::load(&args.config)?;
    let database_path = args
        .database
        .or_else(|| std::env::var_os("CHAT_ROOM_DATABASE_PATH").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("chat_rooms.db"));
    let state = Arc::new(AppState::open_with_config(&database_path, &config).await?);
    let web_enabled = !args.no_web;
    let app = build_app_with_web(state, web_enabled);

    let listener = tokio::net::TcpListener::bind(listen_addr)
        .await
        .with_context(|| format!("bind server to {}", listen_addr))?;
    tracing::info!("listening on http://{}", listen_addr);
    tracing::info!(
        "maximum upload size: {} MiB",
        config.uploads.max_file_size_mib
    );
    if web_enabled {
        tracing::info!("web client enabled at http://{}/", listen_addr);
    }

    axum::serve(listener, app).await.context("serve chat room")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_web_server_options() {
        let args = Args::try_parse_from([
            "server",
            "--web",
            "--listen",
            "127.0.0.1:4321",
            "--database",
            "rooms.sqlite",
            "--config",
            "custom.toml",
        ])
        .unwrap();

        assert!(args.web);
        assert!(!args.no_web);
        assert_eq!(args.listen_addr(), "127.0.0.1:4321".parse().unwrap());
        assert_eq!(args.database, Some(PathBuf::from("rooms.sqlite")));
        assert_eq!(args.config, PathBuf::from("custom.toml"));
    }

    #[test]
    fn port_overrides_listen_port() {
        let args =
            Args::try_parse_from(["server", "--listen", "0.0.0.0:3000", "--port", "8080"]).unwrap();

        assert_eq!(args.listen_addr(), "0.0.0.0:8080".parse().unwrap());
    }
}
