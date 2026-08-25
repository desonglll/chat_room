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
