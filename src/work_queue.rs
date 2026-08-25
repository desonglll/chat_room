//! Fair admission queues for durable message writes and resumable upload work.

use std::{sync::Arc, time::Duration};

use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use crate::config::WorkQueueConfig;

#[derive(Clone)]
pub(crate) struct WorkQueue {
    message: Arc<Semaphore>,
    upload: Arc<Semaphore>,
    wait_timeout: Duration,
}

impl WorkQueue {
    pub fn new(config: &WorkQueueConfig) -> Self {
        Self {
            message: Arc::new(Semaphore::new(config.message_concurrency)),
            upload: Arc::new(Semaphore::new(config.upload_concurrency)),
            wait_timeout: Duration::from_secs(config.wait_timeout_secs),
        }
    }

    pub async fn message(&self) -> Result<OwnedSemaphorePermit, QueueOverloaded> {
        self.acquire(self.message.clone()).await
    }

    pub async fn upload(&self) -> Result<OwnedSemaphorePermit, QueueOverloaded> {
        self.acquire(self.upload.clone()).await
    }

    async fn acquire(
        &self,
        semaphore: Arc<Semaphore>,
    ) -> Result<OwnedSemaphorePermit, QueueOverloaded> {
        tokio::time::timeout(self.wait_timeout, semaphore.acquire_owned())
            .await
            .map_err(|_| QueueOverloaded)?
            .map_err(|_| QueueOverloaded)
    }
}

#[derive(Debug)]
pub(crate) struct QueueOverloaded;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn releases_waiting_message_work_in_order() {
        let queue = WorkQueue::new(&WorkQueueConfig {
            message_concurrency: 1,
            upload_concurrency: 1,
            wait_timeout_secs: 1,
        });
        let permit = queue.message().await.unwrap();
        let waiting = tokio::spawn({
            let queue = queue.clone();
            async move { queue.message().await }
        });
        tokio::task::yield_now().await;
        drop(permit);
        assert!(waiting.await.unwrap().is_ok());
    }
}
