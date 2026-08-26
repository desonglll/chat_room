//! Fair admission queues for durable message writes and resumable upload work.

use std::{sync::Arc, time::Duration};

use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use crate::config::WorkQueueConfig;

#[derive(Clone)]
pub(crate) struct WorkQueue {
    message: Arc<Semaphore>,
    upload: Arc<Semaphore>,
    message_limit: u32,
    upload_limit: u32,
    wait_timeout: Duration,
}

impl WorkQueue {
    pub fn new(config: &WorkQueueConfig) -> Self {
        Self {
            message: Arc::new(Semaphore::new(config.message_concurrency)),
            upload: Arc::new(Semaphore::new(config.upload_concurrency)),
            message_limit: config.message_concurrency as u32,
            upload_limit: config.upload_concurrency as u32,
            wait_timeout: Duration::from_secs(config.wait_timeout_secs),
        }
    }

    pub async fn message(&self) -> Result<OwnedSemaphorePermit, QueueOverloaded> {
        self.acquire(self.message.clone()).await
    }

    pub async fn upload(&self) -> Result<OwnedSemaphorePermit, QueueOverloaded> {
        self.acquire(self.upload.clone()).await
    }

    pub async fn maintenance(&self) -> Result<MaintenancePermits, QueueOverloaded> {
        let message = self
            .acquire_all(self.message.clone(), self.message_limit)
            .await?;
        let upload = self
            .acquire_all(self.upload.clone(), self.upload_limit)
            .await?;
        Ok(MaintenancePermits {
            _message: message,
            _upload: upload,
        })
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

    async fn acquire_all(
        &self,
        semaphore: Arc<Semaphore>,
        permits: u32,
    ) -> Result<OwnedSemaphorePermit, QueueOverloaded> {
        tokio::time::timeout(self.wait_timeout, semaphore.acquire_many_owned(permits))
            .await
            .map_err(|_| QueueOverloaded)?
            .map_err(|_| QueueOverloaded)
    }
}

pub(crate) struct MaintenancePermits {
    _message: OwnedSemaphorePermit,
    _upload: OwnedSemaphorePermit,
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

    #[tokio::test]
    async fn maintenance_waits_for_active_work_and_holds_every_permit() {
        let queue = WorkQueue::new(&WorkQueueConfig {
            message_concurrency: 1,
            upload_concurrency: 1,
            wait_timeout_secs: 1,
        });
        let upload = queue.upload().await.unwrap();
        let waiting = tokio::spawn({
            let queue = queue.clone();
            async move { queue.maintenance().await }
        });
        tokio::task::yield_now().await;
        assert!(!waiting.is_finished());
        drop(upload);
        let maintenance = waiting.await.unwrap().unwrap();
        assert_eq!(queue.message.available_permits(), 0);
        assert_eq!(queue.upload.available_permits(), 0);
        drop(maintenance);
    }
}
