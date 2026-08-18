//! Attachment storage with atomic staging and range reads. In-progress
//! (chunked or single-shot) uploads always stage on local disk; once
//! complete, the durable copy goes either to that same local disk (default)
//! or to Aliyun OSS when `[attachments.oss]` is enabled — see `commit`,
//! `commit_chunked`, `open_range` and `remove` for the branch point.

use std::{path::PathBuf, time::Duration};

use anyhow::{Context, Result};
use opendal::{services, Operator};
use tokio::fs::File;
use tokio::io::{AsyncRead, AsyncSeekExt, AsyncWriteExt, SeekFrom};
use tokio_util::compat::FuturesAsyncReadCompatExt;
use uuid::Uuid;

use crate::config::OssConfig;

#[derive(Clone, Debug)]
pub struct AttachmentStore {
    root: PathBuf,
    staging: PathBuf,
    abandoned_upload_age: Duration,
    oss: Option<Operator>,
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
        let oss = if oss.enabled {
            let builder = services::Oss::default()
                .endpoint(&oss.endpoint)
                .bucket(&oss.bucket)
                .access_key_id(&oss.access_key_id)
                .access_key_secret(&oss.access_key_secret)
                .root(&oss.root);
            Some(Operator::new(builder).context("build Aliyun OSS attachment backend")?)
        } else {
            None
        };
        let store = Self { root, staging, abandoned_upload_age, oss };
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

    /// Commit a completed chunked upload — the counterpart to `commit()` for
    /// staged uploads built via `begin()`/`write()`.
    pub async fn commit_chunked(&self, upload_id: Uuid, attachment_id: Uuid) -> Result<i64> {
        let source = self.chunked_staging_path(upload_id);
        if let Some(operator) = &self.oss {
            let bytes = tokio::fs::read(&source)
                .await
                .with_context(|| format!("read staged chunked upload {}", source.display()))?;
            let size = i64::try_from(bytes.len()).context("attachment is too large")?;
            operator
                .write(&object_key(attachment_id), bytes)
                .await
                .context("upload chunked attachment to OSS")?;
            tokio::fs::remove_file(&source)
                .await
                .with_context(|| format!("remove staged chunked upload {}", source.display()))?;
            return Ok(size);
        }
        let size = tokio::fs::metadata(&source)
            .await
            .with_context(|| format!("stat chunked upload {}", source.display()))?
            .len();
        let target = self.path(attachment_id);
        if let Some(parent) = target.parent() {
            tokio::fs::create_dir_all(parent).await.with_context(|| {
                format!("create attachment shard directory {}", parent.display())
            })?;
        }
        tokio::fs::rename(&source, &target).await.with_context(|| {
            format!(
                "commit chunked attachment {} to {}",
                source.display(),
                target.display()
            )
        })?;
        Ok(i64::try_from(size).context("attachment is too large")?)
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

    pub async fn commit(&self, mut staged: StagedUpload, id: Uuid) -> Result<i64> {
        let size = staged.size;
        let mut file = staged.file.take().context("staged upload has no file")?;
        file.flush().await.context("flush staged attachment")?;
        file.sync_all().await.context("sync staged attachment")?;
        drop(file);
        let source = staged.path.take().context("staged upload has no path")?;
        if let Some(operator) = &self.oss {
            let bytes = tokio::fs::read(&source)
                .await
                .with_context(|| format!("read staged attachment {}", source.display()))?;
            operator
                .write(&object_key(id), bytes)
                .await
                .context("upload attachment to OSS")?;
            tokio::fs::remove_file(&source)
                .await
                .with_context(|| format!("remove staged attachment {}", source.display()))?;
            return Ok(size);
        }
        let target = self.path(id);
        if let Some(parent) = target.parent() {
            tokio::fs::create_dir_all(parent).await.with_context(|| {
                format!("create attachment shard directory {}", parent.display())
            })?;
        }
        tokio::fs::rename(&source, &target).await.with_context(|| {
            format!(
                "commit attachment {} to {}",
                source.display(),
                target.display()
            )
        })?;
        Ok(size)
    }

    pub async fn import_legacy(&self, id: Uuid, bytes: &[u8]) -> Result<()> {
        let target = self.path(id);
        if tokio::fs::metadata(&target)
            .await
            .is_ok_and(|metadata| metadata.len() == bytes.len() as u64)
        {
            return Ok(());
        }
        let mut staged = self.begin().await?;
        staged.write(bytes).await?;
        self.commit(staged, id).await?;
        Ok(())
    }

    pub async fn open_range(
        &self,
        id: Uuid,
        start: u64,
        length: u64,
    ) -> Result<Box<dyn AsyncRead + Send + Unpin>> {
        if let Some(operator) = &self.oss {
            let end = start.checked_add(length).context("range overflow")?;
            let reader = operator
                .reader(&object_key(id))
                .await
                .context("open OSS attachment reader")?
                .into_futures_async_read(start..end)
                .await
                .context("open OSS attachment range")?;
            return Ok(Box::new(reader.compat()));
        }
        let path = self.path(id);
        let mut file = File::open(&path)
            .await
            .with_context(|| format!("open attachment {}", path.display()))?;
        file.seek(SeekFrom::Start(start))
            .await
            .context("seek attachment")?;
        Ok(Box::new(tokio::io::AsyncReadExt::take(file, length)))
    }

    pub async fn remove(&self, id: Uuid) -> Result<()> {
        if let Some(operator) = &self.oss {
            return operator
                .delete(&object_key(id))
                .await
                .context("delete attachment from OSS");
        }
        let path = self.path(id);
        match tokio::fs::remove_file(&path).await {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => {
                Err(error).with_context(|| format!("remove attachment {}", path.display()))
            }
        }
    }

    pub fn path(&self, id: Uuid) -> PathBuf {
        let name = id.simple().to_string();
        self.root.join(&name[..2]).join(name)
    }

    pub fn oss_enabled(&self) -> bool {
        self.oss.is_some()
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
fn object_key(id: Uuid) -> String {
    let name = id.simple().to_string();
    format!("{}/{}", &name[..2], name)
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
            endpoint: "https://oss-cn-hangzhou.aliyuncs.com".into(),
            bucket: "test-bucket".into(),
            access_key_id: "fake-id".into(),
            access_key_secret: "fake-secret".into(),
            root: "/chat-room/".into(),
        };
        let store = AttachmentStore::open(test_directory(), Duration::from_secs(3600), &oss)
            .await
            .expect("building the OSS operator must not require a live connection");
        assert!(store.oss_enabled());
    }
}
