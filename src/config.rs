//! TOML-backed runtime configuration.
//!
//! This file owns only the shape of `[section]` → struct and the order in which
//! the sections are loaded and validated. Every section's fields, defaults and
//! validation rules live in the module that owns the section, so adding a
//! setting never widens this file.
use anyhow::{Context, Result};
use serde::Deserialize;
use std::{io::ErrorKind, path::Path};

use crate::{ai::AiConfig, push_notifications::WebPushConfig};

mod admin;
mod auth;
mod backup;
mod database;
mod environment;
mod map;
mod observability;
mod performance;
mod public;
mod realtime;
mod security;
mod storage;
#[cfg(test)]
mod tests;
mod vector_store;

pub use admin::AdminConfig;
pub use auth::AuthConfig;
pub use backup::BackupConfig;
pub use database::DatabaseConfig;
pub use map::MapConfig;
pub use observability::ObservabilityConfig;
pub use performance::{RedisConfig, WorkQueueConfig};
pub use public::{public_config, PublicConfig};
pub use realtime::RealtimeConfig;
pub use security::SecurityConfig;
pub use storage::{AttachmentConfig, OssConfig, UploadConfig, DEFAULT_MAX_UPLOAD_MIB};
pub use vector_store::VectorStoreConfig;

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub uploads: UploadConfig,
    pub attachments: AttachmentConfig,
    pub database: DatabaseConfig,
    pub ai: AiConfig,
    pub realtime: RealtimeConfig,
    pub auth: AuthConfig,
    pub security: SecurityConfig,
    pub admin: AdminConfig,
    pub redis: RedisConfig,
    pub work_queue: WorkQueueConfig,
    pub vector_store: VectorStoreConfig,
    pub web_push: WebPushConfig,
    pub observability: ObservabilityConfig,
    pub backup: BackupConfig,
    pub map: MapConfig,
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

    /// Delegate to each section. A new section adds one line here and keeps its
    /// rules in its own file; nothing section-specific belongs in this function.
    pub fn validate(self) -> Result<Self> {
        self.uploads.validate()?;
        self.attachments.validate()?;
        self.realtime.validate()?;
        self.auth.validate()?;
        self.security.validate()?;
        self.web_push.validate()?;
        self.observability.validate()?;
        self.backup.validate(self.attachments.oss.enabled)?;
        self.admin.validate()?;
        self.redis.validate()?;
        self.work_queue.validate()?;
        self.database.validate()?;
        self.ai.validate()?;
        self.vector_store.validate()?;
        self.map.validate()?;
        Ok(self)
    }

    pub fn max_upload_bytes(&self) -> Result<usize> {
        self.uploads.max_upload_bytes()
    }

    pub fn chunk_size_bytes(&self) -> Result<usize> {
        self.uploads.chunk_size_bytes()
    }
}
