//! CLI account session storage and authentication requests.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct UserConfig {
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub token: Option<Uuid>,
}

#[derive(Deserialize)]
struct AuthUser {
    username: String,
}

#[derive(Deserialize)]
struct AuthSession {
    token: Uuid,
    user: AuthUser,
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

pub fn history_path() -> PathBuf {
    config_path().with_file_name(".chatroom_history")
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

    let temporary = PathBuf::from(format!("{}.tmp", path.display()));
    let data = serde_json::to_string_pretty(config).context("serialize user config")?;
    std::fs::write(&temporary, data)
        .with_context(|| format!("write config {}", temporary.display()))?;
    std::fs::rename(&temporary, path)
        .with_context(|| format!("install config {}", path.display()))?;
    Ok(())
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
    let response = reqwest::Client::new()
        .post(format!("{http_base}/api/users/{endpoint}"))
        .json(&serde_json::json!({ "username": username, "password": password }))
        .send()
        .await
        .with_context(|| format!("{endpoint} account"))?;

    match response.status().as_u16() {
        200 | 201 => {
            let session: AuthSession = response.json().await.context("decode login session")?;
            let config = UserConfig {
                username: session.user.username,
                token: Some(session.token),
            };
            save_config_to(&config_path(), &config)?;
            println!("Logged in as '{}'.", config.username);
            Ok(())
        }
        400 => bail!("username is invalid or password is shorter than 8 characters"),
        401 => bail!("incorrect username or password"),
        409 => bail!("username already exists"),
        status => bail!("server returned unexpected status {status}"),
    }
}

pub async fn logout(http_base: &str) -> Result<()> {
    let config = load_config()?;
    if let Some(token) = config.token {
        let _ = reqwest::Client::new()
            .post(format!("{http_base}/api/users/logout"))
            .bearer_auth(token)
            .send()
            .await;
    }
    save_config_to(&config_path(), &UserConfig::default())?;
    println!("Logged out.");
    Ok(())
}
