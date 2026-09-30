//! Bootstrap admin roster and the retention windows admin jobs enforce.

use anyhow::{bail, Result};
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct AdminConfig {
    /// Deprecated one-time import of existing accounts into persistent roles.
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

impl AdminConfig {
    pub(super) fn validate(&self) -> Result<()> {
        if self.orphan_retention_hours <= 0 {
            bail!("admin.orphan_retention_hours must be greater than zero");
        }
        if self.deleted_room_retention_days <= 0 {
            bail!("admin.deleted_room_retention_days must be greater than zero");
        }
        if self
            .usernames
            .iter()
            .any(|username| username.trim().is_empty())
        {
            bail!("admin.usernames must not contain empty values");
        }
        Ok(())
    }
}
