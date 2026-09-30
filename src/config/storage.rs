//! Upload limits and the attachment storage backend (local disk or Aliyun OSS).

use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::path::PathBuf;

pub const DEFAULT_MAX_UPLOAD_MIB: u64 = 512;
const BYTES_PER_MIB: u64 = 1024 * 1024;

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

impl UploadConfig {
    pub(super) fn validate(&self) -> Result<()> {
        if self.max_file_size_mib == 0 {
            bail!("uploads.max_file_size_mib must be greater than zero");
        }
        if self.chunk_size_mib == 0 {
            bail!("uploads.chunk_size_mib must be greater than zero");
        }
        // Both conversions are part of validation: a MiB value that overflows
        // `usize` must fail at startup, not on the first upload.
        self.max_upload_bytes()?;
        self.chunk_size_bytes()?;
        Ok(())
    }

    pub(super) fn max_upload_bytes(&self) -> Result<usize> {
        let bytes = self
            .max_file_size_mib
            .checked_mul(BYTES_PER_MIB)
            .context("uploads.max_file_size_mib is too large")?;
        usize::try_from(bytes).context("uploads.max_file_size_mib exceeds this platform's limit")
    }

    pub(super) fn chunk_size_bytes(&self) -> Result<usize> {
        let bytes = self
            .chunk_size_mib
            .checked_mul(BYTES_PER_MIB)
            .context("uploads.chunk_size_mib is too large")?;
        usize::try_from(bytes).context("uploads.chunk_size_mib exceeds this platform's limit")
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

impl AttachmentConfig {
    pub(super) fn validate(&self) -> Result<()> {
        if self.directory.as_os_str().is_empty() {
            bail!("attachments.directory must not be empty");
        }
        self.oss.validate()
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

impl OssConfig {
    fn validate(&self) -> Result<()> {
        if self.enabled {
            if self.endpoint.trim().is_empty() {
                bail!("attachments.oss.endpoint is required when attachments.oss.enabled is true");
            }
            if self.bucket.trim().is_empty() {
                bail!("attachments.oss.bucket is required when attachments.oss.enabled is true");
            }
            if self.access_key_id.trim().is_empty() {
                bail!(
                    "attachments.oss.access_key_id is required when attachments.oss.enabled is true"
                );
            }
            if self.access_key_secret.trim().is_empty() {
                bail!("attachments.oss.access_key_secret is required when attachments.oss.enabled is true");
            }
            // Not validated here: whether these credentials actually work —
            // consistent with [ai], a bad OSS config shouldn't crash the whole
            // server at startup; it surfaces as upload/download failures instead.
        }
        if self.direct_upload_enabled && !self.enabled {
            bail!("attachments.oss.direct_upload_enabled requires attachments.oss.enabled");
        }
        if !(60..=3600).contains(&self.presign_expiry_secs) {
            bail!("attachments.oss.presign_expiry_secs must be between 60 and 3600");
        }
        if !(1..=300).contains(&self.operation_timeout_secs) {
            bail!("attachments.oss.operation_timeout_secs must be between 1 and 300");
        }
        if !matches!(
            self.presign_addressing_style.as_str(),
            "virtual" | "path" | "cname"
        ) {
            bail!("attachments.oss.presign_addressing_style must be virtual, path, or cname");
        }
        Ok(())
    }
}
