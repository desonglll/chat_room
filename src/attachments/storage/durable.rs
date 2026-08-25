//! Durable local/OSS publication, reads, and cleanup.

use std::path::Path;

use anyhow::{Context, Result};
use futures_util::TryStreamExt;
use opendal::{ErrorKind, Operator};
use sha2::{Digest, Sha256};
use tokio::fs::File;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncSeekExt, AsyncWriteExt, SeekFrom};
use tokio_util::compat::FuturesAsyncReadCompatExt;
use uuid::Uuid;

use super::{object_key, AttachmentStore, StagedUpload};

const OSS_WRITE_CHUNK_BYTES: usize = 4 * 1024 * 1024;

impl AttachmentStore {
    pub async fn commit_chunked(&self, upload_id: Uuid, key: &str) -> Result<i64> {
        let source = self.chunked_staging_path(upload_id);
        self.commit_path(&source, key).await
    }

    pub async fn commit(&self, mut staged: StagedUpload, key: &str) -> Result<i64> {
        let mut file = staged.file.take().context("staged upload has no file")?;
        file.flush().await.context("flush staged attachment")?;
        file.sync_all().await.context("sync staged attachment")?;
        drop(file);
        let source = staged.path.take().context("staged upload has no path")?;
        self.commit_path(&source, key).await
    }

    async fn commit_path(&self, source: &Path, key: &str) -> Result<i64> {
        let size = tokio::fs::metadata(source)
            .await
            .with_context(|| format!("stat staged attachment {}", source.display()))?
            .len();
        if let Some(operator) = &self.oss {
            if self.local_mirror_enabled {
                let target = self.prepare_local_target(key).await?;
                publish_local(source, &target).await?;
                match upload_path_to_oss(operator, key, &target, self.oss_operation_timeout).await {
                    Ok(()) => {}
                    Err(error) => tracing::warn!(
                        storage_key = key,
                        "OSS attachment write failed; retained local mirror: {error:#}"
                    ),
                }
            } else {
                upload_path_to_oss(operator, key, source, self.oss_operation_timeout).await?;
                tokio::fs::remove_file(source)
                    .await
                    .with_context(|| format!("remove staged attachment {}", source.display()))?;
            }
        } else {
            let target = self.prepare_local_target(key).await?;
            publish_local(source, &target).await?;
        }
        i64::try_from(size).context("attachment is too large")
    }

    pub(super) async fn publish_local_staged(
        &self,
        mut staged: StagedUpload,
        key: &str,
    ) -> Result<i64> {
        let size = staged.size;
        let mut file = staged.file.take().context("staged upload has no file")?;
        file.flush().await.context("flush staged attachment")?;
        file.sync_all().await.context("sync staged attachment")?;
        drop(file);
        let source = staged.path.take().context("staged upload has no path")?;
        let target = self.prepare_local_target(key).await?;
        publish_local(&source, &target).await?;
        Ok(size)
    }

    async fn prepare_local_target(&self, key: &str) -> Result<std::path::PathBuf> {
        let target = self.path(key);
        if let Some(parent) = target.parent() {
            tokio::fs::create_dir_all(parent).await.with_context(|| {
                format!("create attachment shard directory {}", parent.display())
            })?;
        }
        Ok(target)
    }

    pub async fn open_range(
        &self,
        key: &str,
        start: u64,
        length: u64,
    ) -> Result<Box<dyn AsyncRead + Send + Unpin>> {
        if let Some(operator) = &self.oss {
            if self.local_mirror_enabled {
                let stat = tokio::time::timeout(
                    self.oss_operation_timeout,
                    operator.stat(&object_key(key)),
                )
                .await;
                if !matches!(stat, Ok(Ok(_))) {
                    tracing::warn!(
                        storage_key = key,
                        "OSS attachment is unavailable; using local mirror"
                    );
                    return self.open_local_range(key, start, length).await;
                }
            }
            match tokio::time::timeout(
                self.oss_operation_timeout,
                open_oss_range(operator, key, start, length),
            )
            .await
            {
                Ok(Ok(reader)) => return Ok(reader),
                Ok(Err(oss_error)) if self.local_mirror_enabled => {
                    tracing::warn!(
                        storage_key = key,
                        "OSS attachment read failed; using local mirror: {oss_error:#}"
                    );
                    return self
                        .open_local_range(key, start, length)
                        .await
                        .with_context(|| {
                            format!("OSS read also failed before local fallback: {oss_error:#}")
                        });
                }
                Err(_) if self.local_mirror_enabled => {
                    tracing::warn!(
                        storage_key = key,
                        "OSS attachment read timed out; using local mirror"
                    );
                    return self.open_local_range(key, start, length).await;
                }
                Ok(Err(error)) => return Err(error),
                Err(_) => return Err(anyhow::anyhow!("open OSS attachment timed out")),
            }
        }
        self.open_local_range(key, start, length).await
    }

    async fn open_local_range(
        &self,
        key: &str,
        start: u64,
        length: u64,
    ) -> Result<Box<dyn AsyncRead + Send + Unpin>> {
        let path = self.path(key);
        let mut file = File::open(&path)
            .await
            .with_context(|| format!("open attachment {}", path.display()))?;
        file.seek(SeekFrom::Start(start))
            .await
            .context("seek attachment")?;
        Ok(Box::new(tokio::io::AsyncReadExt::take(file, length)))
    }

    pub async fn hash_stored(&self, key: &str) -> Result<String> {
        if self.oss.is_none() {
            return super::sha256_hex_of_path(&self.path(key)).await;
        }
        if self.local_mirror_enabled && local_exists(&self.path(key)).await? {
            return super::sha256_hex_of_path(&self.path(key)).await;
        }
        sha256_hex_of_oss(
            self.oss.as_ref().expect("OSS checked above"),
            key,
            self.oss_operation_timeout,
        )
        .await
    }

    pub async fn exists(&self, key: &str) -> Result<bool> {
        if let Some(operator) = &self.oss {
            let oss_result = match tokio::time::timeout(
                self.oss_operation_timeout,
                operator.stat(&object_key(key)),
            )
            .await
            {
                Ok(Ok(_)) => Ok(true),
                Ok(Err(error)) if error.kind() == ErrorKind::NotFound => Ok(false),
                Ok(Err(error)) => Err(error).context("stat OSS attachment"),
                Err(_) => Err(anyhow::anyhow!("stat OSS attachment timed out")),
            };
            if matches!(oss_result, Ok(true)) || !self.local_mirror_enabled {
                return oss_result;
            }
            let local = local_exists(&self.path(key)).await?;
            return if local { Ok(true) } else { oss_result };
        }
        local_exists(&self.path(key)).await
    }

    pub async fn remove(&self, key: &str) -> Result<()> {
        let oss_error = if let Some(operator) = &self.oss {
            match tokio::time::timeout(
                self.oss_operation_timeout,
                operator.delete(&object_key(key)),
            )
            .await
            {
                Ok(Ok(())) => None,
                Ok(Err(error)) => {
                    Some(anyhow::Error::new(error).context("delete attachment from OSS"))
                }
                Err(_) => Some(anyhow::anyhow!("delete attachment from OSS timed out")),
            }
        } else {
            None
        };
        if self.oss.is_none() || self.local_mirror_enabled {
            remove_local(&self.path(key)).await?;
        }
        match oss_error {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }
}

async fn open_oss_range(
    operator: &Operator,
    key: &str,
    start: u64,
    length: u64,
) -> Result<Box<dyn AsyncRead + Send + Unpin>> {
    let end = start.checked_add(length).context("range overflow")?;
    let reader = operator
        .reader(&object_key(key))
        .await
        .context("open OSS attachment reader")?
        .into_futures_async_read(start..end)
        .await
        .context("open OSS attachment range")?;
    Ok(Box::new(reader.compat()))
}

pub(super) async fn publish_local(source: &Path, target: &Path) -> Result<()> {
    if tokio::fs::metadata(target).await.is_ok() {
        tokio::fs::remove_file(source)
            .await
            .with_context(|| format!("discard duplicate attachment {}", source.display()))?;
        return Ok(());
    }
    tokio::fs::rename(source, target).await.with_context(|| {
        format!(
            "commit attachment {} to {}",
            source.display(),
            target.display()
        )
    })
}

async fn upload_path_to_oss(
    operator: &Operator,
    key: &str,
    path: &Path,
    operation_timeout: std::time::Duration,
) -> Result<()> {
    let mut source = File::open(path)
        .await
        .with_context(|| format!("open staged attachment {}", path.display()))?;
    let mut writer = tokio::time::timeout(
        operation_timeout,
        operator
            .writer_with(&object_key(key))
            .chunk(OSS_WRITE_CHUNK_BYTES),
    )
    .await
    .context("open OSS attachment writer timed out")?
    .context("open OSS attachment writer")?;
    let mut buffer = vec![0u8; OSS_WRITE_CHUNK_BYTES];
    loop {
        let read = source
            .read(&mut buffer)
            .await
            .with_context(|| format!("read staged attachment {}", path.display()))?;
        if read == 0 {
            break;
        }
        tokio::time::timeout(operation_timeout, writer.write(buffer[..read].to_vec()))
            .await
            .context("stream attachment to OSS timed out")?
            .context("stream attachment to OSS")?;
    }
    tokio::time::timeout(operation_timeout, writer.close())
        .await
        .context("finish OSS attachment upload timed out")?
        .context("finish OSS attachment upload")?;
    Ok(())
}

async fn sha256_hex_of_oss(
    operator: &Operator,
    key: &str,
    operation_timeout: std::time::Duration,
) -> Result<String> {
    let reader = tokio::time::timeout(operation_timeout, operator.reader(&object_key(key)))
        .await
        .context("open OSS attachment for hashing timed out")?
        .context("open OSS attachment for hashing")?;
    let mut stream = tokio::time::timeout(operation_timeout, reader.into_stream(..))
        .await
        .context("stream OSS attachment for hashing timed out")?
        .context("stream OSS attachment for hashing")?;
    let mut hasher = Sha256::new();
    loop {
        let buffer = tokio::time::timeout(operation_timeout, stream.try_next())
            .await
            .context("read OSS attachment for hashing timed out")?
            .context("read OSS attachment for hashing")?;
        let Some(buffer) = buffer else { break };
        for bytes in buffer {
            hasher.update(&bytes);
        }
    }
    Ok(hex::encode(hasher.finalize()))
}

async fn local_exists(path: &Path) -> Result<bool> {
    match tokio::fs::metadata(path).await {
        Ok(metadata) => Ok(metadata.is_file()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error).context("stat local attachment"),
    }
}

async fn remove_local(path: &Path) -> Result<()> {
    match tokio::fs::remove_file(path).await {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).with_context(|| format!("remove attachment {}", path.display())),
    }
}
