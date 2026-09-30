//! WebSocket timing and history/poll limits.

use anyhow::{bail, Result};
use serde::Deserialize;

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

impl RealtimeConfig {
    pub(super) fn validate(&self) -> Result<()> {
        if self.poll_interval_ms == 0 {
            bail!("realtime.poll_interval_ms must be greater than zero");
        }
        if self.history_replay_limit <= 0 {
            bail!("realtime.history_replay_limit must be greater than zero");
        }
        if self.message_poll_limit <= 0 {
            bail!("realtime.message_poll_limit must be greater than zero");
        }
        Ok(())
    }
}
