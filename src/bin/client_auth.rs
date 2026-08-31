//! CLI account session storage and authentication requests.

use std::{
    io::Write,
    path::{Path, PathBuf},
};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::client_api::ApiClient;

#[derive(Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct UserConfig {
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub token: Option<Uuid>,
}

pub fn config_path() -> PathBuf {
    if let Ok(path) = std::env::var("CHAT_ROOM_CONFIG_PATH") {
        return PathBuf::from(path);
    }
    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home).join(".chatroom.conf");
    }
    PathBuf::from("chatroom.conf")
}

pub fn load_config_from(path: &Path) -> Result<UserConfig> {
    if !path.exists() {
        return Ok(UserConfig::default());
    }
    let data =
        std::fs::read_to_string(path).with_context(|| format!("read config {}", path.display()))?;
    serde_json::from_str(&data).with_context(|| format!("parse config {}", path.display()))
}

pub fn save_config_to(path: &Path, config: &UserConfig) -> Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create config directory {}", parent.display()))?;
        }
    }

    let temporary = PathBuf::from(format!("{}.{}.tmp", path.display(), Uuid::new_v4()));
    let data = serde_json::to_string_pretty(config).context("serialize user config")?;
    let result = (|| {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .with_context(|| format!("create config {}", temporary.display()))?;
        file.write_all(data.as_bytes())
            .with_context(|| format!("write config {}", temporary.display()))?;
        file.sync_all()
            .with_context(|| format!("sync config {}", temporary.display()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&temporary, std::fs::Permissions::from_mode(0o600))
                .with_context(|| format!("protect config {}", temporary.display()))?;
        }
        std::fs::rename(&temporary, path)
            .with_context(|| format!("install config {}", path.display()))
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result
}

pub fn load_config() -> Result<UserConfig> {
    load_config_from(&config_path())
}

pub fn require_session() -> Result<UserConfig> {
    let config = load_config()?;
    if config.username.is_empty() || config.token.is_none() {
        bail!("not logged in; run 'client login' or 'client register'");
    }
    Ok(config)
}

pub fn show_config() -> Result<()> {
    let config = load_config()?;
    if config.username.is_empty() || config.token.is_none() {
        println!("Not logged in.");
    } else {
        println!("Logged in as '{}'.", config.username);
    }
    Ok(())
}

pub async fn authenticate(
    http_base: &str,
    endpoint: &str,
    username: &str,
    password: &str,
) -> Result<()> {
    let session = ApiClient::new(http_base, None)
        .authenticate(endpoint == "register", username, password)
        .await?;
    let config = UserConfig {
        username: session.user.username,
        token: Some(session.token),
    };
    save_config_to(&config_path(), &config)?;
    println!("Logged in as '{}'.", config.username);
    Ok(())
}

pub async fn logout(http_base: &str) -> Result<()> {
    let config = load_config()?;
    if let Some(token) = config.token {
        let _ = ApiClient::new(http_base, Some(token)).logout().await;
    }
    save_config_to(&config_path(), &UserConfig::default())?;
    println!("Logged out.");
    Ok(())
}
