//! Server binary with optional embedded web client.

use std::{net::SocketAddr, path::PathBuf, sync::Arc};

use anyhow::{Context, Result};
use chat_room::{backup, build_app_with_web, config::AppConfig, state::AppState};
use clap::{Parser, Subcommand, ValueEnum};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Debug, Parser)]
#[command(name = "server", about = "SQLite/PostgreSQL chat room server")]
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

    /// Database type. Overrides database.kind in the TOML configuration.
    #[arg(long, value_enum, global = true)]
    database_type: Option<DatabaseType>,

    /// SQLite path or PostgreSQL URL, according to --database-type.
    #[arg(long, value_name = "PATH_OR_URL", global = true)]
    database: Option<String>,

    /// TOML configuration path. A missing file uses built-in defaults.
    #[arg(
        long,
        default_value = "chat-room.toml",
        value_name = "PATH",
        global = true
    )]
    config: PathBuf,

    #[command(subcommand)]
    command: Option<MaintenanceCommand>,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum DatabaseType {
    Sqlite,
    Postgres,
}

#[derive(Debug, Subcommand)]
enum MaintenanceCommand {
    /// Export a complete PostgreSQL dump and all local attachment files.
    Export {
        /// New directory to create for the backup.
        #[arg(short, long, value_name = "DIRECTORY")]
        output: PathBuf,
    },
    /// Restore PostgreSQL and local attachments from a verified backup.
    Restore {
        /// Backup directory containing manifest.json.
        #[arg(short, long, value_name = "DIRECTORY")]
        input: PathBuf,
    },
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
    let database_type = args.database_type.unwrap_or_else(|| {
        if config.database.kind == "postgres" {
            DatabaseType::Postgres
        } else {
            DatabaseType::Sqlite
        }
    });
    if let Some(command) = &args.command {
        if !matches!(database_type, DatabaseType::Postgres) {
            anyhow::bail!("export and restore require PostgreSQL; select --database-type postgres");
        }
        let url = postgres_url(&args, &config)?;
        match command {
            MaintenanceCommand::Export { output } => {
                let manifest = backup::export_postgres(&config, &url, output).await?;
                let bytes: u64 = manifest.files.iter().map(|file| file.size_bytes).sum();
                println!(
                    "backup created at {} ({} files, {} bytes)",
                    output.display(),
                    manifest.files.len(),
                    bytes
                );
            }
            MaintenanceCommand::Restore { input } => {
                let outcome = backup::restore_postgres(&config, &url, input).await?;
                println!("backup restored from {}", input.display());
                if let Some(previous) = outcome.previous_attachments {
                    println!("previous attachments preserved at {}", previous.display());
                }
                if config.redis.enabled {
                    println!("cleared {} Redis cache keys", outcome.redis_keys_cleared);
                }
            }
        }
        return Ok(());
    }
    let state = Arc::new(match database_type {
        DatabaseType::Sqlite => {
            let path = args
                .database
                .or_else(|| std::env::var("CHAT_ROOM_DATABASE_PATH").ok())
                .map(PathBuf::from)
                .unwrap_or_else(|| config.database.sqlite_path.clone());
            AppState::open_with_config(&path, &config).await?
        }
        DatabaseType::Postgres => {
            let url = postgres_url(&args, &config)?;
            AppState::open_postgres(&url, &config).await?
        }
    });
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

fn postgres_url(args: &Args, config: &AppConfig) -> Result<String> {
    let url = args
        .database
        .clone()
        .or_else(|| std::env::var("CHAT_ROOM_DATABASE_URL").ok())
        .unwrap_or_else(|| config.database.postgres_url.clone());
    if url.trim().is_empty() {
        anyhow::bail!(
            "PostgreSQL requires --database URL, CHAT_ROOM_DATABASE_URL, or database.postgres_url"
        );
    }
    Ok(url)
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
            "--database-type",
            "sqlite",
            "--config",
            "custom.toml",
        ])
        .unwrap();

        assert!(args.web);
        assert!(!args.no_web);
        assert_eq!(args.listen_addr(), "127.0.0.1:4321".parse().unwrap());
        assert_eq!(args.database.as_deref(), Some("rooms.sqlite"));
        assert!(matches!(args.database_type, Some(DatabaseType::Sqlite)));
        assert_eq!(args.config, PathBuf::from("custom.toml"));
    }

    #[test]
    fn port_overrides_listen_port() {
        let args =
            Args::try_parse_from(["server", "--listen", "0.0.0.0:3000", "--port", "8080"]).unwrap();

        assert_eq!(args.listen_addr(), "0.0.0.0:8080".parse().unwrap());
    }

    #[test]
    fn parses_backup_commands_with_global_database_options() {
        let export = Args::try_parse_from([
            "server",
            "export",
            "--output",
            "backups/complete",
            "--database-type",
            "postgres",
            "--database",
            "postgres://localhost/chat",
        ])
        .unwrap();
        assert!(matches!(
            export.command,
            Some(MaintenanceCommand::Export { output })
                if output.as_path() == std::path::Path::new("backups/complete")
        ));

        let restore =
            Args::try_parse_from(["server", "restore", "--input", "backups/complete"]).unwrap();
        assert!(matches!(
            restore.command,
            Some(MaintenanceCommand::Restore { input })
                if input.as_path() == std::path::Path::new("backups/complete")
        ));
    }
}
