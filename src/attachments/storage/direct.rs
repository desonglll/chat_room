//! Short-lived browser uploads and validation of their OSS objects.

use std::collections::BTreeMap;

use anyhow::{bail, Context, Result};
use chrono::{DateTime, Utc};
use futures_util::TryStreamExt;
use serde::Serialize;
use sha2::{Digest, Sha256};
use utoipa::ToSchema;
use uuid::Uuid;

use super::{object_key, AttachmentStore};

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct DirectUploadTarget {
    pub method: String,
    pub url: String,
    pub headers: BTreeMap<String, String>,
    pub expires_at: DateTime<Utc>,
}

impl AttachmentStore {
    pub async fn presign_upload(
        &self,
        upload_id: Uuid,
        mime_type: &str,
    ) -> Result<Option<DirectUploadTarget>> {
        let Some(expiry) = self.direct_upload_expiry else {
            return Ok(None);
        };
        let operator = self.oss.as_ref().context("OSS is not configured")?;
        let signed = operator
            .presign_write_with(&object_key(&direct_storage_key(upload_id)), expiry)
            .content_type(mime_type)
            .await
            .context("sign direct OSS attachment upload")?;
        let mut headers = BTreeMap::new();
        if let Some(value) = signed.header().get(axum::http::header::CONTENT_TYPE) {
            headers.insert(
                "content-type".into(),
                value
                    .to_str()
                    .context("signed OSS Content-Type is not valid text")?
                    .to_owned(),
            );
        }
        let expires_at = Utc::now()
            + chrono::Duration::from_std(expiry).context("OSS upload expiry is too large")?;
        Ok(Some(DirectUploadTarget {
            method: signed.method().to_string(),
            url: signed.uri().to_string(),
            headers,
            expires_at,
        }))
    }

    /// Verify size and SHA-256 before a direct object becomes message content.
    pub async fn commit_direct(
        &self,
        upload_id: Uuid,
        expected_hash: &str,
        expected_size: i64,
    ) -> Result<String> {
        let operator = self.oss.as_ref().context("OSS is not configured")?;
        let temporary_key = direct_storage_key(upload_id);
        let oss_key = object_key(&temporary_key);
        let metadata = tokio::time::timeout(self.oss_operation_timeout, operator.stat(&oss_key))
            .await
            .context("stat direct OSS attachment timed out")?
            .context("stat direct OSS attachment")?;
        if metadata.content_length() != expected_size as u64 {
            let _ =
                tokio::time::timeout(self.oss_operation_timeout, operator.delete(&oss_key)).await;
            bail!(
                "direct OSS attachment size mismatch: expected {expected_size}, got {}",
                metadata.content_length()
            );
        }

        let reader = tokio::time::timeout(self.oss_operation_timeout, operator.reader(&oss_key))
            .await
            .context("open direct OSS attachment timed out")?
            .context("open direct OSS attachment")?;
        let mut stream = tokio::time::timeout(self.oss_operation_timeout, reader.into_stream(..))
            .await
            .context("stream direct OSS attachment timed out")?
            .context("stream direct OSS attachment")?;
        let mut hasher = Sha256::new();
        let mut local = if self.local_mirror_enabled {
            Some(self.begin().await?)
        } else {
            None
        };
        loop {
            let buffer = tokio::time::timeout(self.oss_operation_timeout, stream.try_next())
                .await
                .context("read direct OSS attachment timed out")?
                .context("read direct OSS attachment")?;
            let Some(buffer) = buffer else { break };
            for bytes in buffer {
                hasher.update(&bytes);
                if let Some(staged) = &mut local {
                    staged.write(&bytes).await?;
                }
            }
        }
        let actual_hash = hex::encode(hasher.finalize());
        if actual_hash != expected_hash {
            let _ =
                tokio::time::timeout(self.oss_operation_timeout, operator.delete(&oss_key)).await;
            bail!("direct OSS attachment does not match its declared SHA-256");
        }
        let storage_key = format!("df{}", Uuid::new_v4().simple());
        if let Some(staged) = local {
            self.publish_local_staged(staged, &storage_key).await?;
        }
        let copied = tokio::time::timeout(
            self.oss_operation_timeout,
            operator.copy(&oss_key, &object_key(&storage_key)),
        )
        .await;
        let promotion = match copied {
            Ok(Ok(_)) => Ok(()),
            Ok(Err(error)) if self.local_mirror_enabled => {
                tracing::warn!(
                    storage_key,
                    "direct OSS promotion failed; retained local mirror: {error:#}"
                );
                Ok(())
            }
            Err(_) if self.local_mirror_enabled => {
                tracing::warn!(
                    storage_key,
                    "direct OSS promotion timed out; retained local mirror"
                );
                Ok(())
            }
            Ok(Err(error)) => {
                Err(anyhow::Error::new(error).context("promote direct OSS attachment"))
            }
            Err(_) => Err(anyhow::anyhow!("promote direct OSS attachment timed out")),
        };
        let _ = tokio::time::timeout(self.oss_operation_timeout, operator.delete(&oss_key)).await;
        promotion?;
        Ok(storage_key)
    }

    pub async fn discard_direct(&self, upload_id: Uuid) -> Result<()> {
        if self.direct_upload_expiry.is_none() {
            return Ok(());
        }
        let Some(operator) = &self.oss else {
            return Ok(());
        };
        let storage_key = direct_storage_key(upload_id);
        let oss_error = match tokio::time::timeout(
            self.oss_operation_timeout,
            operator.delete(&object_key(&storage_key)),
        )
        .await
        {
            Ok(Ok(())) => None,
            Ok(Err(error)) => {
                Some(anyhow::Error::new(error).context("delete direct OSS attachment"))
            }
            Err(_) => Some(anyhow::anyhow!("delete direct OSS attachment timed out")),
        };
        if self.local_mirror_enabled {
            let path = self.path(&storage_key);
            match tokio::fs::remove_file(&path).await {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error).context("delete direct local mirror"),
            }
        }
        match oss_error {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }
}

fn direct_storage_key(upload_id: Uuid) -> String {
    format!("du{}", upload_id.simple())
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use opendal::{services, Operator};
    use sha2::{Digest, Sha256};

    use super::*;

    async fn direct_store() -> (AttachmentStore, Operator) {
        let root = super::super::test_directory();
        let staging = root.join(".staging");
        tokio::fs::create_dir_all(&staging).await.unwrap();
        let remote_root = super::super::test_directory();
        tokio::fs::create_dir_all(&remote_root).await.unwrap();
        let operator = Operator::new(
            services::Fs::default().root(remote_root.to_str().expect("UTF-8 test path")),
        )
        .unwrap();
        let store = AttachmentStore {
            root,
            staging,
            abandoned_upload_age: Duration::from_secs(3600),
            oss: Some(operator.clone()),
            local_mirror_enabled: true,
            direct_upload_expiry: Some(Duration::from_secs(900)),
            oss_operation_timeout: Duration::from_secs(5),
        };
        (store, operator)
    }

    #[tokio::test]
    async fn direct_object_is_hashed_before_local_publication() {
        let (store, remote) = direct_store().await;
        let upload_id = Uuid::new_v4();
        let bytes = b"verified direct object";
        let storage_key = direct_storage_key(upload_id);
        remote
            .write(&object_key(&storage_key), bytes.to_vec())
            .await
            .unwrap();
        let hash = hex::encode(Sha256::digest(bytes));

        let committed = store
            .commit_direct(upload_id, &hash, bytes.len() as i64)
            .await
            .unwrap();

        assert_ne!(committed, storage_key);
        assert_eq!(
            tokio::fs::read(store.path(&committed)).await.unwrap(),
            bytes
        );
        assert!(remote.exists(&object_key(&committed)).await.unwrap());
        assert!(!remote.exists(&object_key(&storage_key)).await.unwrap());
    }

    #[tokio::test]
    async fn direct_object_with_false_hash_is_deleted() {
        let (store, remote) = direct_store().await;
        let upload_id = Uuid::new_v4();
        let storage_key = direct_storage_key(upload_id);
        remote
            .write(&object_key(&storage_key), b"tampered".to_vec())
            .await
            .unwrap();

        let result = store.commit_direct(upload_id, &"0".repeat(64), 8).await;

        assert!(result.is_err());
        assert!(!remote.exists(&object_key(&storage_key)).await.unwrap());
        assert!(tokio::fs::metadata(store.path(&storage_key)).await.is_err());
    }
}
