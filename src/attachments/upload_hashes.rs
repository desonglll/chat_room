//! Incremental SHA-256 state for active chunked uploads.

use std::collections::HashMap;

use sha2::{Digest, Sha256};
use tokio::sync::Mutex;
use uuid::Uuid;

struct UploadHash {
    received_bytes: u64,
    hasher: Sha256,
}

#[derive(Default)]
pub(crate) struct UploadHashTracker {
    uploads: Mutex<HashMap<Uuid, UploadHash>>,
}

impl UploadHashTracker {
    pub async fn record(&self, upload_id: Uuid, offset: u64, bytes: &[u8]) {
        let mut uploads = self.uploads.lock().await;
        if offset == 0 {
            uploads.entry(upload_id).or_insert_with(|| UploadHash {
                received_bytes: 0,
                hasher: Sha256::new(),
            });
        }
        let Some(upload) = uploads.get_mut(&upload_id) else {
            return;
        };
        if upload.received_bytes != offset {
            uploads.remove(&upload_id);
            return;
        }
        upload.hasher.update(bytes);
        upload.received_bytes += bytes.len() as u64;
    }

    pub async fn completed_digest(&self, upload_id: Uuid, expected_size: u64) -> Option<String> {
        let uploads = self.uploads.lock().await;
        let upload = uploads.get(&upload_id)?;
        (upload.received_bytes == expected_size)
            .then(|| hex::encode(upload.hasher.clone().finalize()))
    }

    pub async fn remove(&self, upload_id: Uuid) {
        self.uploads.lock().await.remove(&upload_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn hashes_contiguous_chunks() {
        let tracker = UploadHashTracker::default();
        let id = Uuid::new_v4();
        tracker.record(id, 0, b"a").await;
        tracker.record(id, 1, b"bc").await;
        assert_eq!(
            tracker.completed_digest(id, 3).await.as_deref(),
            Some("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad")
        );
    }

    #[tokio::test]
    async fn refuses_a_non_contiguous_digest() {
        let tracker = UploadHashTracker::default();
        let id = Uuid::new_v4();
        tracker.record(id, 0, b"a").await;
        tracker.record(id, 2, b"c").await;
        assert_eq!(tracker.completed_digest(id, 3).await, None);
    }
}
