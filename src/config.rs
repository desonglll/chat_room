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
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct UploadConfig {
    pub max_file_size_mib: u64,
}

impl Default for UploadConfig {
    fn default() -> Self {
        Self {
            max_file_size_mib: DEFAULT_MAX_UPLOAD_MIB,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct AttachmentConfig {
    pub directory: PathBuf,
}

impl Default for AttachmentConfig {
    fn default() -> Self {
        Self {
            directory: PathBuf::from("chat_attachments"),
        }
    }
}

impl AppConfig {
    pub fn load(path: &Path) -> Result<Self> {
        let source = match std::fs::read_to_string(path) {
            Ok(source) => source,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(Self::default()),
            Err(error) => return Err(error).with_context(|| format!("read {}", path.display())),
        };
        toml::from_str::<Self>(&source)
            .with_context(|| format!("parse TOML configuration {}", path.display()))?
            .validate()
    }

    pub fn validate(self) -> Result<Self> {
        if self.uploads.max_file_size_mib == 0 {
            bail!("uploads.max_file_size_mib must be greater than zero");
        }
        if self.attachments.directory.as_os_str().is_empty() {
            bail!("attachments.directory must not be empty");
        }
        self.max_upload_bytes()?;
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
}

#[derive(Serialize)]
pub struct PublicConfig {
    max_upload_bytes: usize,
}

pub async fn public_config(State(state): State<SharedState>) -> Json<PublicConfig> {
    Json(PublicConfig {
        max_upload_bytes: state.max_upload_bytes(),
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
}
