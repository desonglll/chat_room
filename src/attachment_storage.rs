//! Filesystem-backed attachment storage with atomic staging and range reads.

use std::{path::PathBuf, time::Duration};

use anyhow::{Context, Result};
use tokio::fs::File;
use tokio::io::{AsyncSeekExt, AsyncWriteExt, SeekFrom, Take};
use uuid::Uuid;

const ABANDONED_UPLOAD_AGE: Duration = Duration::from_secs(24 * 60 * 60);

#[derive(Clone, Debug)]
pub struct AttachmentStore {
    root: PathBuf,
    staging: PathBuf,
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
    pub async fn open(root: impl Into<PathBuf>) -> Result<Self> {
        let root = root.into();
        let staging = root.join(".staging");
        tokio::fs::create_dir_all(&staging)
            .await
            .with_context(|| format!("create attachment directory {}", staging.display()))?;
        let store = Self { root, staging };
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

    pub async fn commit(&self, mut staged: StagedUpload, id: Uuid) -> Result<i64> {
        let size = staged.size;
        let mut file = staged.file.take().context("staged upload has no file")?;
        file.flush().await.context("flush staged attachment")?;
        file.sync_all().await.context("sync staged attachment")?;
        drop(file);
        let source = staged.path.take().context("staged upload has no path")?;
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

    pub async fn open_range(&self, id: Uuid, start: u64, length: u64) -> Result<Take<File>> {
        let path = self.path(id);
        let mut file = File::open(&path)
            .await
            .with_context(|| format!("open attachment {}", path.display()))?;
        file.seek(SeekFrom::Start(start))
            .await
            .context("seek attachment")?;
        Ok(tokio::io::AsyncReadExt::take(file, length))
    }

    pub async fn remove(&self, id: Uuid) -> Result<()> {
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
                .is_some_and(|age| age >= ABANDONED_UPLOAD_AGE);
            if metadata.is_file() && abandoned {
                tokio::fs::remove_file(&path)
                    .await
                    .with_context(|| format!("remove abandoned upload {}", path.display()))?;
            }
        }
        Ok(())
    }
}

pub fn test_directory() -> PathBuf {
    std::env::temp_dir()
        .join("chat-room-tests")
        .join(Uuid::new_v4().simple().to_string())
}
