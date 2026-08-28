use std::path::PathBuf;

use anyhow::{bail, Result};
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct BackupConfig {
    pub enabled: bool,
    pub interval_minutes: u64,
    pub retention_count: usize,
    pub target_backend: String,
    pub directory: PathBuf,
    pub include_files: bool,
}

impl Default for BackupConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            interval_minutes: 24 * 60,
            retention_count: 7,
            target_backend: "local".into(),
            directory: PathBuf::from("chat_backups"),
            include_files: false,
        }
    }
}

impl BackupConfig {
    pub(super) fn validate(&self, oss_enabled: bool) -> Result<()> {
        if self.interval_minutes == 0 {
            bail!("backup.interval_minutes must be greater than zero");
        }
        if self.retention_count == 0 || self.retention_count > 10_000 {
            bail!("backup.retention_count must be between 1 and 10000");
        }
        if self.target_backend != "local" {
            bail!("backup.target_backend currently supports only local");
        }
        if self.directory.as_os_str().is_empty() {
            bail!("backup.directory must not be empty");
        }
        if self.enabled && self.include_files && oss_enabled {
            bail!("backup.include_files is unavailable with OSS attachments");
        }
        Ok(())
    }
}
