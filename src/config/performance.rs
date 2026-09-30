//! Redis fan-out and the bounded work queues that shed load ahead of it.

use anyhow::{bail, Result};
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct WorkQueueConfig {
    pub message_concurrency: usize,
    pub upload_concurrency: usize,
    pub wait_timeout_secs: u64,
}

impl Default for WorkQueueConfig {
    fn default() -> Self {
        Self {
            message_concurrency: 32,
            upload_concurrency: 4,
            wait_timeout_secs: 30,
        }
    }
}

impl WorkQueueConfig {
    pub(super) fn validate(&self) -> Result<()> {
        if self.message_concurrency == 0
            || self.upload_concurrency == 0
            || self.message_concurrency > u32::MAX as usize
            || self.upload_concurrency > u32::MAX as usize
        {
            bail!("work_queue concurrency limits must be between 1 and u32::MAX");
        }
        if self.wait_timeout_secs == 0 || self.wait_timeout_secs > 300 {
            bail!("work_queue.wait_timeout_secs must be between 1 and 300");
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct RedisConfig {
    pub enabled: bool,
    pub url: String,
    pub key_prefix: String,
    pub connect_timeout_ms: u64,
    pub command_timeout_ms: u64,
    pub message_ttl_secs: u64,
}

impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            url: "redis://127.0.0.1:6379/".into(),
            key_prefix: "chat-room".into(),
            connect_timeout_ms: 1500,
            command_timeout_ms: 500,
            message_ttl_secs: 30,
        }
    }
}

impl RedisConfig {
    pub(super) fn validate(&self) -> Result<()> {
        if self.enabled && self.url.trim().is_empty() {
            bail!("redis.url is required when redis.enabled is true");
        }
        if self.key_prefix.trim().is_empty() {
            bail!("redis.key_prefix must not be empty");
        }
        if self.connect_timeout_ms == 0 || self.command_timeout_ms == 0 {
            bail!("redis timeouts must be greater than zero");
        }
        if self.message_ttl_secs == 0 || self.message_ttl_secs > 3600 {
            bail!("redis.message_ttl_secs must be between 1 and 3600");
        }
        Ok(())
    }
}
