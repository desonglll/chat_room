//! TOML-backed runtime configuration and its public browser-safe projection.

use std::{
    io::ErrorKind,
    path::{Path, PathBuf},
};

use crate::{
    ai::{AiConfig, AiRuntimeStatus},
    state::SharedState,
};
use anyhow::{bail, Context, Result};
use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};

mod environment;
mod knowledge_graph;
mod performance;
#[cfg(test)]
mod tests;
mod vector_store;

pub use knowledge_graph::KnowledgeGraphConfig;
pub use performance::{RedisConfig, WorkQueueConfig};
pub use vector_store::VectorStoreConfig;

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
    pub work_queue: WorkQueueConfig,
    pub vector_store: VectorStoreConfig,
    pub knowledge_graph: KnowledgeGraphConfig,
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
    /// Keep a durable local copy and use it when the OSS object is unavailable.
    pub local_mirror_enabled: bool,
    /// Let browsers upload directly with a short-lived, object-scoped PUT URL.
    pub direct_upload_enabled: bool,
    pub presign_expiry_secs: u64,
    pub operation_timeout_secs: u64,
    /// Optional browser-reachable endpoint when the server uses an internal endpoint.
    pub presign_endpoint: String,
    /// OpenDAL addressing style for `presign_endpoint`: virtual, path, or cname.
    pub presign_addressing_style: String,
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
            local_mirror_enabled: false,
            direct_upload_enabled: false,
            presign_expiry_secs: 900,
            operation_timeout_secs: 30,
            presign_endpoint: String::new(),
            presign_addressing_style: "virtual".into(),
            endpoint: String::new(),
            bucket: String::new(),
            access_key_id: String::new(),
            access_key_secret: String::new(),
            root: "/".into(),
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
        environment::apply(&mut config);
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
        if self.attachments.oss.direct_upload_enabled && !self.attachments.oss.enabled {
            bail!("attachments.oss.direct_upload_enabled requires attachments.oss.enabled");
        }
        if !(60..=3600).contains(&self.attachments.oss.presign_expiry_secs) {
            bail!("attachments.oss.presign_expiry_secs must be between 60 and 3600");
        }
        if !(1..=300).contains(&self.attachments.oss.operation_timeout_secs) {
            bail!("attachments.oss.operation_timeout_secs must be between 1 and 300");
        }
        if !matches!(
            self.attachments.oss.presign_addressing_style.as_str(),
            "virtual" | "path" | "cname"
        ) {
            bail!("attachments.oss.presign_addressing_style must be virtual, path, or cname");
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
        if self.redis.message_ttl_secs == 0 || self.redis.message_ttl_secs > 3600 {
            bail!("redis.message_ttl_secs must be between 1 and 3600");
        }
        if self.work_queue.message_concurrency == 0 || self.work_queue.upload_concurrency == 0 {
            bail!("work_queue concurrency limits must be greater than zero");
        }
        if self.work_queue.wait_timeout_secs == 0 || self.work_queue.wait_timeout_secs > 300 {
            bail!("work_queue.wait_timeout_secs must be between 1 and 300");
        }
        for (name, body) in [
            ("ai.standard_extra_body", &self.ai.standard_extra_body),
            ("ai.reasoning_extra_body", &self.ai.reasoning_extra_body),
        ] {
            if body.as_ref().is_some_and(|value| !value.is_object()) {
                bail!("{name} must be a JSON/TOML object");
            }
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
            if self.ai.max_context_messages == 0 || self.ai.analysis_context_messages == 0 {
                bail!("AI context message limits must be greater than zero");
            }
            if self.ai.request_timeout_secs == 0 || self.ai.request_timeout_secs > 300 {
                bail!("ai.request_timeout_secs must be between 1 and 300");
            }
            if self.ai.stream_idle_timeout_secs == 0 || self.ai.stream_idle_timeout_secs > 300 {
                bail!("ai.stream_idle_timeout_secs must be between 1 and 300");
            }
            if self.ai.stream_total_timeout_secs < self.ai.stream_idle_timeout_secs
                || self.ai.stream_total_timeout_secs > 1800
            {
                bail!("ai.stream_total_timeout_secs must be between the idle timeout and 1800");
            }
            // Credential availability is reported at runtime so an optional
            // AI misconfiguration does not prevent the chat server starting.
        }
        if self.vector_store.enabled {
            if self.vector_store.url.trim().is_empty()
                || self.vector_store.collection.trim().is_empty()
                || self.vector_store.embedding_base_url.trim().is_empty()
                || self.vector_store.embedding_model.trim().is_empty()
            {
                bail!("vector_store requires url, collection, embedding_base_url, and embedding_model when enabled");
            }
            if self.vector_store.dimensions == 0 || self.vector_store.dimensions > 65_536 {
                bail!("vector_store.dimensions must be between 1 and 65536");
            }
            if self.vector_store.top_k == 0 || self.vector_store.top_k > 50 {
                bail!("vector_store.top_k must be between 1 and 50");
            }
            if !(0.0..=1.0).contains(&self.vector_store.score_threshold) {
                bail!("vector_store.score_threshold must be between 0 and 1");
            }
            if self.vector_store.worker_interval_ms == 0 {
                bail!("vector_store.worker_interval_ms must be greater than zero");
            }
            if !self.vector_store.collection.chars().all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '_' | '-')
            }) {
                bail!(
                    "vector_store.collection may contain only ASCII letters, numbers, '_' and '-'"
                );
            }
        }
        if self.knowledge_graph.enabled {
            if self.knowledge_graph.url.trim().is_empty()
                || self.knowledge_graph.api_token_env.trim().is_empty()
            {
                bail!("knowledge_graph requires url and api_token_env when enabled");
            }
            if self.knowledge_graph.max_facts == 0 || self.knowledge_graph.max_facts > 50 {
                bail!("knowledge_graph.max_facts must be between 1 and 50");
            }
            if self.knowledge_graph.graph_limit == 0 || self.knowledge_graph.graph_limit > 1_000 {
                bail!("knowledge_graph.graph_limit must be between 1 and 1000");
            }
            if self.knowledge_graph.worker_interval_ms == 0
                || self.knowledge_graph.request_timeout_secs == 0
                || self.knowledge_graph.search_timeout_ms == 0
                || self.knowledge_graph.worker_concurrency == 0
            {
                bail!("knowledge_graph timeouts and worker concurrency must be greater than zero");
            }
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
    ai_status: AiRuntimeStatus,
    knowledge_graph_enabled: bool,
}

pub async fn public_config(State(state): State<SharedState>) -> Json<PublicConfig> {
    let choices = state.ai_model_choices().await.unwrap_or_default();
    let ai_status = if choices.iter().any(|choice| choice.ready) {
        AiRuntimeStatus::Ready
    } else if choices.is_empty() {
        AiRuntimeStatus::Disabled
    } else {
        AiRuntimeStatus::MissingCredentials
    };
    Json(PublicConfig {
        max_upload_bytes: state.max_upload_bytes(),
        ai_enabled: ai_status == AiRuntimeStatus::Ready,
        ai_status,
        knowledge_graph_enabled: state.knowledge_graph().is_some(),
    })
}
