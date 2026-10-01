//! TG-408 link previews. On by default; every fetch goes through the SSRF policy in
//! `messages::link_previews::ssrf`.

use std::net::SocketAddr;

use anyhow::{bail, Result};
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct LinkPreviewConfig {
    pub enabled: bool,
    /// TEST FIXTURES ONLY: exact `ip:port` sockets exempt from the private-address refusal (and
    /// their ports from the port allow-list). Never set this in a deployment.
    pub unsafe_allow_sockets: Vec<String>,
}

impl Default for LinkPreviewConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            unsafe_allow_sockets: Vec::new(),
        }
    }
}

impl LinkPreviewConfig {
    pub fn allowed_sockets(&self) -> Vec<SocketAddr> {
        self.unsafe_allow_sockets
            .iter()
            .filter_map(|value| value.parse().ok())
            .collect()
    }

    pub(super) fn validate(&self) -> Result<()> {
        if self.allowed_sockets().len() != self.unsafe_allow_sockets.len() {
            bail!("link_preview.unsafe_allow_sockets must be ip:port socket addresses");
        }
        Ok(())
    }
}
