//! Which database adapter the server runs on, and how it connects.

use anyhow::{bail, Result};
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct DatabaseConfig {
    pub kind: String,
    pub sqlite_path: PathBuf,
    pub postgres_url: String,
    pub max_connections: u32,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            kind: "sqlite".into(),
            sqlite_path: PathBuf::from("chat_rooms.db"),
            postgres_url: String::new(),
            max_connections: 10,
        }
    }
}

impl DatabaseConfig {
    pub(super) fn validate(&self) -> Result<()> {
        if !matches!(self.kind.as_str(), "sqlite" | "postgres") {
            bail!("database.kind must be 'sqlite' or 'postgres'");
        }
        if self.max_connections == 0 {
            bail!("database.max_connections must be greater than zero");
        }
        if self.kind == "postgres" && self.postgres_url.trim().is_empty() {
            bail!("database.postgres_url is required when database.kind is 'postgres'");
        }
        Ok(())
    }
}
