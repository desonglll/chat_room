//! TOML-backed runtime configuration and its public browser-safe projection.

use std::{
    io::ErrorKind,
    path::{Path, PathBuf},
};

use anyhow::{bail, Context, Result};
use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};

use crate::state::SharedState;

pub const DEFAULT_MAX_UPLOAD_MIB: u64 = 512;
const BYTES_PER_MIB: u64 = 1024 * 1024;

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub uploads: UploadConfig,
    pub attachments: AttachmentConfig,
    pub database: DatabaseConfig,
    pub ai: AiConfig,
    pub realtime: RealtimeConfig,
    pub auth: AuthConfig,
    pub admin: AdminConfig,
    pub redis: RedisConfig,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct RedisConfig {
    pub enabled: bool,
    pub url: String,
    pub key_prefix: String,
    pub connect_timeout_ms: u64,
    pub command_timeout_ms: u64,
}

impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            url: "redis://127.0.0.1:6379/".into(),
            key_prefix: "chat-room".into(),
            connect_timeout_ms: 1500,
            command_timeout_ms: 500,
        }
    }
}

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

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct UploadConfig {
    pub max_file_size_mib: u64,
    /// Per-request body cap for one chunk of a resumable upload — independent
    /// of how large the whole file is.
    pub chunk_size_mib: u64,
    /// How long an abandoned in-progress upload's staged bytes are kept
    /// before being garbage-collected.
    pub abandoned_upload_gc_hours: u64,
}

impl Default for UploadConfig {
    fn default() -> Self {
        Self {
            max_file_size_mib: DEFAULT_MAX_UPLOAD_MIB,
            chunk_size_mib: 8,
            abandoned_upload_gc_hours: 24,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct AttachmentConfig {
    /// Local staging directory — always used for in-progress (chunked) uploads
    /// regardless of `oss.enabled`, and for the final attachment bytes too
    /// when OSS is off.
    pub directory: PathBuf,
    pub oss: OssConfig,
}

impl Default for AttachmentConfig {
    fn default() -> Self {
        Self {
            directory: PathBuf::from("chat_attachments"),
            oss: OssConfig::default(),
        }
    }
}

/// Aliyun OSS as the durable attachment backend, selected instead of local
/// disk when enabled. Disabled by default — local disk keeps working exactly
/// as before either way.
#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct OssConfig {
    pub enabled: bool,
    pub endpoint: String,
    pub bucket: String,
    pub access_key_id: String,
    pub access_key_secret: String,
    /// Key prefix inside the bucket.
    pub root: String,
}

impl Default for OssConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            endpoint: String::new(),
            bucket: String::new(),
            access_key_id: String::new(),
            access_key_secret: String::new(),
            root: "/".into(),
        }
    }
}

/// AI "magic button" (summarize + suggest replies). Disabled unless explicitly
/// turned on. The API key itself is never stored in TOML or exposed to the
/// client — only the *name* of the environment variable holding it.
#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct AiConfig {
    pub enabled: bool,
    /// "openai" or "anthropic" — validated, but dispatch itself is inferred by
    /// the `genai` crate from `model`'s name prefix (e.g. "gpt-*", "claude-*").
    pub provider: String,
    pub api_key_env: String,
    pub model: String,
    pub base_url: Option<String>,
    pub max_context_messages: usize,
    /// Per-user cooldown between AI suggestion requests in the same room.
    pub suggest_cooldown_secs: u64,
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            provider: "openai".into(),
            api_key_env: String::new(),
            model: String::new(),
            base_url: None,
            max_context_messages: 30,
            suggest_cooldown_secs: 10,
        }
    }
}

/// WebSocket timing and history/poll limits.
#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct RealtimeConfig {
    pub poll_interval_ms: u64,
    pub heartbeat_interval_secs: u64,
    pub auth_timeout_secs: u64,
    pub history_replay_limit: i64,
    pub message_poll_limit: i64,
    /// Per-(room, from, target) cooldown between pokes.
    pub poke_cooldown_secs: u64,
}

impl Default for RealtimeConfig {
    fn default() -> Self {
        Self {
            poll_interval_ms: 250,
            heartbeat_interval_secs: 15,
            auth_timeout_secs: 10,
            history_replay_limit: 100,
            message_poll_limit: 200,
            poke_cooldown_secs: 5,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct AuthConfig {
    pub session_lifetime_days: i64,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            session_lifetime_days: 30,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct AdminConfig {
    /// Case-insensitive usernames allowed to access system-wide operations.
    pub usernames: Vec<String>,
    pub orphan_retention_hours: i64,
    pub deleted_room_retention_days: i64,
}

impl Default for AdminConfig {
    fn default() -> Self {
        Self {
            usernames: Vec::new(),
            orphan_retention_hours: 168,
            deleted_room_retention_days: 30,
        }
    }
}

impl AppConfig {
    pub fn load(path: &Path) -> Result<Self> {
        let mut config = match std::fs::read_to_string(path) {
            Ok(source) => toml::from_str::<Self>(&source)
                .with_context(|| format!("parse TOML configuration {}", path.display()))?,
            Err(error) if error.kind() == ErrorKind::NotFound => Self::default(),
            Err(error) => return Err(error).with_context(|| format!("read {}", path.display())),
        };
        if let Ok(usernames) = std::env::var("CHAT_ROOM_ADMIN_USERNAMES") {
            config.admin.usernames = usernames
                .split(',')
                .map(str::trim)
                .filter(|username| !username.is_empty())
                .map(str::to_string)
                .collect();
        }
        if let Ok(url) = std::env::var("CHAT_ROOM_REDIS_URL") {
            config.redis.enabled = !url.trim().is_empty();
            config.redis.url = url;
        }
        config.validate()
    }

    pub fn validate(self) -> Result<Self> {
        if self.uploads.max_file_size_mib == 0 {
            bail!("uploads.max_file_size_mib must be greater than zero");
        }
        if self.uploads.chunk_size_mib == 0 {
            bail!("uploads.chunk_size_mib must be greater than zero");
        }
        if self.attachments.directory.as_os_str().is_empty() {
            bail!("attachments.directory must not be empty");
        }
        if self.attachments.oss.enabled {
            if self.attachments.oss.endpoint.trim().is_empty() {
                bail!("attachments.oss.endpoint is required when attachments.oss.enabled is true");
            }
            if self.attachments.oss.bucket.trim().is_empty() {
                bail!("attachments.oss.bucket is required when attachments.oss.enabled is true");
            }
            if self.attachments.oss.access_key_id.trim().is_empty() {
                bail!("attachments.oss.access_key_id is required when attachments.oss.enabled is true");
            }
            if self.attachments.oss.access_key_secret.trim().is_empty() {
                bail!("attachments.oss.access_key_secret is required when attachments.oss.enabled is true");
            }
            // Not validated here: whether these credentials actually work —
            // consistent with [ai], a bad OSS config shouldn't crash the whole
            // server at startup; it surfaces as upload/download failures instead.
        }
        if self.realtime.poll_interval_ms == 0 {
            bail!("realtime.poll_interval_ms must be greater than zero");
        }
        if self.realtime.history_replay_limit <= 0 {
            bail!("realtime.history_replay_limit must be greater than zero");
        }
        if self.realtime.message_poll_limit <= 0 {
            bail!("realtime.message_poll_limit must be greater than zero");
        }
        if self.auth.session_lifetime_days <= 0 {
            bail!("auth.session_lifetime_days must be greater than zero");
        }
        if self.admin.orphan_retention_hours <= 0 {
            bail!("admin.orphan_retention_hours must be greater than zero");
        }
        if self.admin.deleted_room_retention_days <= 0 {
            bail!("admin.deleted_room_retention_days must be greater than zero");
        }
        if self.redis.enabled && self.redis.url.trim().is_empty() {
            bail!("redis.url is required when redis.enabled is true");
        }
        if self.redis.key_prefix.trim().is_empty() {
            bail!("redis.key_prefix must not be empty");
        }
        if self.redis.connect_timeout_ms == 0 || self.redis.command_timeout_ms == 0 {
            bail!("redis timeouts must be greater than zero");
        }
        if self
            .admin
            .usernames
            .iter()
            .any(|username| username.trim().is_empty())
        {
            bail!("admin.usernames must not contain empty values");
        }
        if !matches!(self.database.kind.as_str(), "sqlite" | "postgres") {
            bail!("database.kind must be 'sqlite' or 'postgres'");
        }
        if self.database.max_connections == 0 {
            bail!("database.max_connections must be greater than zero");
        }
        if self.database.kind == "postgres" && self.database.postgres_url.trim().is_empty() {
            bail!("database.postgres_url is required when database.kind is 'postgres'");
        }
        if self.ai.enabled {
            if !matches!(self.ai.provider.as_str(), "openai" | "anthropic") {
                bail!("ai.provider must be 'openai' or 'anthropic'");
            }
            if self.ai.api_key_env.trim().is_empty() {
                bail!("ai.api_key_env is required when ai.enabled is true");
            }
            if self.ai.model.trim().is_empty() {
                bail!("ai.model is required when ai.enabled is true");
            }
            // Not validated here: whether `api_key_env` resolves to a set
            // environment variable. It may instead be the literal API key
            // (see AiAssistant::new), so an unresolved name isn't an error —
            // and a hard/soft key check shouldn't block the whole server
            // from starting over an AI misconfiguration.
        }
        self.max_upload_bytes()?;
        self.chunk_size_bytes()?;
        Ok(self)
    }

    pub fn max_upload_bytes(&self) -> Result<usize> {
        let bytes = self
            .uploads
            .max_file_size_mib
            .checked_mul(BYTES_PER_MIB)
            .context("uploads.max_file_size_mib is too large")?;
        usize::try_from(bytes).context("uploads.max_file_size_mib exceeds this platform's limit")
    }

    pub fn chunk_size_bytes(&self) -> Result<usize> {
        let bytes = self
            .uploads
            .chunk_size_mib
            .checked_mul(BYTES_PER_MIB)
            .context("uploads.chunk_size_mib is too large")?;
        usize::try_from(bytes).context("uploads.chunk_size_mib exceeds this platform's limit")
    }
}

#[derive(Serialize)]
pub struct PublicConfig {
    max_upload_bytes: usize,
    ai_enabled: bool,
}

pub async fn public_config(State(state): State<SharedState>) -> Json<PublicConfig> {
    Json(PublicConfig {
        max_upload_bytes: state.max_upload_bytes(),
        ai_enabled: state.ai_enabled(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_upload_limit_and_rejects_zero() {
        let config: AppConfig = toml::from_str(
            "[uploads]\nmax_file_size_mib = 128\n[attachments]\ndirectory = 'files'",
        )
        .unwrap();
        assert_eq!(config.attachments.directory, PathBuf::from("files"));
        assert_eq!(
            config.validate().unwrap().max_upload_bytes().unwrap(),
            128 * 1024 * 1024
        );

        let invalid: AppConfig = toml::from_str("[uploads]\nmax_file_size_mib = 0").unwrap();
        assert!(invalid.validate().is_err());
    }

    #[test]
    fn defaults_match_the_previous_hardcoded_constants() {
        let config = AppConfig::default();
        assert_eq!(config.realtime.poll_interval_ms, 250);
        assert_eq!(config.realtime.heartbeat_interval_secs, 15);
        assert_eq!(config.realtime.auth_timeout_secs, 10);
        assert_eq!(config.realtime.history_replay_limit, 100);
        assert_eq!(config.realtime.message_poll_limit, 200);
        assert_eq!(config.realtime.poke_cooldown_secs, 5);
        assert_eq!(config.auth.session_lifetime_days, 30);
        assert!(config.admin.usernames.is_empty());
        assert_eq!(config.admin.orphan_retention_hours, 168);
        assert_eq!(config.ai.suggest_cooldown_secs, 10);
        assert_eq!(config.uploads.chunk_size_mib, 8);
        assert_eq!(config.uploads.abandoned_upload_gc_hours, 24);
    }

    #[test]
    fn parses_realtime_and_auth_overrides() {
        let parsed: AppConfig = toml::from_str(
            "[realtime]\npoll_interval_ms = 500\nmessage_poll_limit = 50\n[auth]\nsession_lifetime_days = 7",
        )
        .unwrap();
        let config = parsed.validate().unwrap();
        assert_eq!(config.realtime.poll_interval_ms, 500);
        assert_eq!(config.realtime.message_poll_limit, 50);
        assert_eq!(config.auth.session_lifetime_days, 7);
        // Untouched fields keep their defaults.
        assert_eq!(config.realtime.heartbeat_interval_secs, 15);

        let invalid: AppConfig = toml::from_str("[auth]\nsession_lifetime_days = 0").unwrap();
        assert!(invalid.validate().is_err());
    }

    #[test]
    fn oss_attachment_backend_is_disabled_by_default_and_validated_when_enabled() {
        let default = AppConfig::default();
        assert!(!default.attachments.oss.enabled);
        assert!(default.validate().is_ok());

        let missing_bucket: AppConfig = toml::from_str(
            "[attachments.oss]\nenabled = true\nendpoint = 'https://oss-cn-hangzhou.aliyuncs.com'\naccess_key_id = 'id'\naccess_key_secret = 'secret'",
        )
        .unwrap();
        assert!(missing_bucket.validate().is_err());

        let complete: AppConfig = toml::from_str(
            "[attachments.oss]\nenabled = true\nendpoint = 'https://oss-cn-hangzhou.aliyuncs.com'\nbucket = 'my-bucket'\naccess_key_id = 'id'\naccess_key_secret = 'secret'",
        )
        .unwrap();
        assert!(complete.validate().is_ok());
    }

    #[test]
    fn ai_config_does_not_require_a_resolvable_env_var_at_load_time() {
        // api_key_env may be a literal key pasted straight into the TOML rather
        // than an environment variable name (see AiAssistant::new) — startup
        // must not depend on that name coincidentally being set as a real
        // env var, since a misconfigured AI section shouldn't crash the server.
        let config: AppConfig = toml::from_str(
            "[ai]\nenabled = true\nprovider = 'openai'\napi_key_env = 'sk-not-a-real-env-var-name'\nmodel = 'gpt-4o-mini'",
        )
        .unwrap();
        assert!(config.validate().is_ok());

        let missing_model: AppConfig = toml::from_str(
            "[ai]\nenabled = true\nprovider = 'openai'\napi_key_env = 'X'\nmodel = ''",
        )
        .unwrap();
        assert!(missing_model.validate().is_err());

        let bad_provider: AppConfig = toml::from_str(
            "[ai]\nenabled = true\nprovider = 'bogus'\napi_key_env = 'X'\nmodel = 'm'",
        )
        .unwrap();
        assert!(bad_provider.validate().is_err());
    }
}
