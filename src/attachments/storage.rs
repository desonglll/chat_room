//! Attachment storage with atomic staging and range reads. In-progress
//! (chunked or single-shot) uploads always stage on local disk; once
//! complete, the durable storage policy is handled by the `durable` module;
//! short-lived browser upload signatures are handled by `direct`.

mod direct;
mod durable;

pub use direct::DirectUploadTarget;

use std::{path::Path, path::PathBuf, time::Duration};

use anyhow::{Context, Result};
use opendal::{services, Operator};
use sha2::{Digest, Sha256};
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt, SeekFrom};
use uuid::Uuid;

use crate::config::OssConfig;

#[derive(Clone, Debug)]
pub struct AttachmentStore {
    root: PathBuf,
    staging: PathBuf,
    abandoned_upload_age: Duration,
    oss: Option<Operator>,
    local_mirror_enabled: bool,
    direct_upload_expiry: Option<Duration>,
    oss_operation_timeout: Duration,
}

pub struct StagedUpload {
    path: Option<PathBuf>,
    file: Option<File>,
    size: i64,
}

impl Drop for StagedUpload {
    fn drop(&mut self) {
        if let Some(path) = self.path.take() {
            let _ = std::fs::remove_file(path);
        }
    }
}

impl StagedUpload {
    pub fn size(&self) -> i64 {
        self.size
    }

    pub async fn write(&mut self, bytes: &[u8]) -> Result<()> {
        self.file
            .as_mut()
            .context("staged upload is already committed")?
            .write_all(bytes)
            .await
            .context("write staged attachment")?;
        self.size = self
            .size
            .checked_add(i64::try_from(bytes.len()).context("attachment is too large")?)
            .context("attachment size overflow")?;
        Ok(())
    }
}

impl AttachmentStore {
    pub async fn open(
        root: impl Into<PathBuf>,
        abandoned_upload_age: Duration,
        oss: &OssConfig,
    ) -> Result<Self> {
        let root = root.into();
        let staging = root.join(".staging");
        tokio::fs::create_dir_all(&staging)
            .await
            .with_context(|| format!("create attachment directory {}", staging.display()))?;
        let operator = if oss.enabled {
            let mut builder = services::Oss::default()
                .endpoint(&oss.endpoint)
                .bucket(&oss.bucket)
                .access_key_id(&oss.access_key_id)
                .access_key_secret(&oss.access_key_secret)
                .root(&oss.root);
            if !oss.presign_endpoint.trim().is_empty() {
                builder = builder
                    .presign_endpoint(&oss.presign_endpoint)
                    .presign_addressing_style(&oss.presign_addressing_style);
            }
            Some(Operator::new(builder).context("build Aliyun OSS attachment backend")?)
        } else {
            None
        };
        let store = Self {
            root,
            staging,
            abandoned_upload_age,
            oss: operator,
            local_mirror_enabled: oss.enabled && oss.local_mirror_enabled,
            direct_upload_expiry: (oss.enabled && oss.direct_upload_enabled)
                .then(|| Duration::from_secs(oss.presign_expiry_secs)),
            oss_operation_timeout: Duration::from_secs(oss.operation_timeout_secs),
        };
        store.clear_staging().await?;
        Ok(store)
    }

    pub async fn begin(&self) -> Result<StagedUpload> {
        let path = self.staging.join(format!("{}.upload", Uuid::new_v4()));
        let file = File::create(&path)
            .await
            .with_context(|| format!("create staged attachment {}", path.display()))?;
        Ok(StagedUpload {
            path: Some(path),
            file: Some(file),
            size: 0,
        })
    }

    /// Deterministic staging path for a chunked upload session, so each chunk
    /// request (a separate HTTP call, possibly handled by a different task) can
    /// reopen and append to the same file rather than holding state in memory.
    fn chunked_staging_path(&self, upload_id: Uuid) -> PathBuf {
        self.staging.join(format!("{}.chunked", upload_id.simple()))
    }

    /// Append bytes at `offset` to a chunked upload's staging file, creating it on
    /// the first call. Returns the new total size. Errors if `offset` doesn't match
    /// the file's current length — the caller uses that to detect/resume correctly.
    pub async fn append_chunk(&self, upload_id: Uuid, offset: u64, bytes: &[u8]) -> Result<u64> {
        let path = self.chunked_staging_path(upload_id);
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&path)
            .await
            .with_context(|| format!("open chunked upload {}", path.display()))?;
        let current_len = file
            .metadata()
            .await
            .with_context(|| format!("stat chunked upload {}", path.display()))?
            .len();
        if current_len != offset {
            anyhow::bail!("offset mismatch: expected {current_len}, got {offset}");
        }
        file.seek(SeekFrom::Start(offset))
            .await
            .context("seek chunked upload")?;
        file.write_all(bytes).await.context("write upload chunk")?;
        file.flush().await.context("flush upload chunk")?;
        Ok(offset + bytes.len() as u64)
    }

    pub async fn chunked_upload_size(&self, upload_id: Uuid) -> Result<u64> {
        let path = self.chunked_staging_path(upload_id);
        match tokio::fs::metadata(&path).await {
            Ok(metadata) => Ok(metadata.len()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(0),
            Err(error) => {
                Err(error).with_context(|| format!("stat chunked upload {}", path.display()))
            }
        }
    }

    /// Hash a chunked upload's staging file without moving or committing it.
    pub async fn hash_chunked(&self, upload_id: Uuid) -> Result<String> {
        sha256_hex_of_path(&self.chunked_staging_path(upload_id)).await
    }

    pub async fn discard_chunked(&self, upload_id: Uuid) -> Result<()> {
        let path = self.chunked_staging_path(upload_id);
        match tokio::fs::remove_file(&path).await {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => {
                Err(error).with_context(|| format!("remove chunked upload {}", path.display()))
            }
        }
    }

    /// Flush a staged (single-shot) upload to disk and compute its content
    /// hash, without moving or committing it anywhere yet. Safe to call
    /// before deciding — via a DB lookup — whether `commit()` is even needed.
    pub async fn hash_staged(&self, staged: &mut StagedUpload) -> Result<String> {
        let file = staged.file.as_mut().context("staged upload has no file")?;
        file.flush().await.context("flush staged attachment")?;
        file.sync_all().await.context("sync staged attachment")?;
        let path = staged.path.as_ref().context("staged upload has no path")?;
        sha256_hex_of_path(path).await
    }

    pub async fn import_legacy(&self, id: Uuid, bytes: &[u8]) -> Result<()> {
        let key = id.simple().to_string();
        let target = self.path(&key);
        if tokio::fs::metadata(&target)
            .await
            .is_ok_and(|metadata| metadata.len() == bytes.len() as u64)
        {
            return Ok(());
        }
        let mut staged = self.begin().await?;
        staged.write(bytes).await?;
        self.commit(staged, &key).await?;
        Ok(())
    }

    pub fn path(&self, key: &str) -> PathBuf {
        self.root.join(&key[..2]).join(key)
    }

    pub fn oss_enabled(&self) -> bool {
        self.oss.is_some()
    }

    pub fn direct_upload_enabled(&self) -> bool {
        self.direct_upload_expiry.is_some()
    }

    async fn clear_staging(&self) -> Result<()> {
        let mut entries = tokio::fs::read_dir(&self.staging)
            .await
            .with_context(|| format!("read staging directory {}", self.staging.display()))?;
        while let Some(entry) = entries
            .next_entry()
            .await
            .context("read staged attachment")?
        {
            let path = entry.path();
            let metadata = entry.metadata().await?;
            let abandoned = metadata
                .modified()
                .ok()
                .and_then(|modified| modified.elapsed().ok())
                .is_some_and(|age| age >= self.abandoned_upload_age);
            if metadata.is_file() && abandoned {
                tokio::fs::remove_file(&path)
                    .await
                    .with_context(|| format!("remove abandoned upload {}", path.display()))?;
            }
        }
        Ok(())
    }
}

/// Same sharding scheme as the local `path()` helper, as an OSS object key.
pub(super) fn object_key(key: &str) -> String {
    format!("{}/{}", &key[..2], key)
}

/// Stream a file through SHA-256 without loading it fully into memory.
async fn sha256_hex_of_path(path: &Path) -> Result<String> {
    let mut file = File::open(path)
        .await
        .with_context(|| format!("open {} for hashing", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 65536];
    loop {
        let read = file
            .read(&mut buf)
            .await
            .with_context(|| format!("read {} for hashing", path.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buf[..read]);
    }
    Ok(hex::encode(hasher.finalize()))
}

pub fn test_directory() -> PathBuf {
    std::env::temp_dir()
        .join("chat-room-tests")
        .join(Uuid::new_v4().simple().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Building the OSS-backed Operator is local/lazy — it never dials the
    /// network — so this can run without real credentials, and still catches
    /// a bad builder wiring (e.g. a typo'd method name or missing config).
    #[tokio::test]
    async fn opens_with_oss_backend_configured_without_network_access() {
        let oss = OssConfig {
            enabled: true,
            local_mirror_enabled: false,
            endpoint: "https://oss-cn-hangzhou.aliyuncs.com".into(),
            bucket: "test-bucket".into(),
            access_key_id: "fake-id".into(),
            access_key_secret: "fake-secret".into(),
            root: "/chat-room/".into(),
            ..OssConfig::default()
        };
        let store = AttachmentStore::open(test_directory(), Duration::from_secs(3600), &oss)
            .await
            .expect("building the OSS operator must not require a live connection");
        assert!(store.oss_enabled());
    }
}
