use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::Result;
use futures_util::{stream, StreamExt};

use super::store::IndexJob;
use crate::state::SharedState;

pub fn ensure_worker(state: SharedState) {
    if state.message_index().is_none()
        || state
            .message_index_worker_started
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
    {
        return;
    }
    tokio::spawn(async move {
        let interval = state
            .message_index()
            .expect("message index checked above")
            .worker_interval;
        loop {
            if state.maintenance_active() {
                tokio::time::sleep(interval).await;
                continue;
            }
            let index = state
                .message_index()
                .expect("message index checked before worker startup");
            if let Err(error) = index.ensure_ready().await {
                if rejects_credentials(&error) {
                    tracing::error!(
                        "vector store rejected the configured credentials; automatic indexing \
                         stopped until the server restarts"
                    );
                    return;
                }
                tracing::warn!(
                    "vector store unavailable; automatic indexing will retry: {error:#}"
                );
                tokio::time::sleep(interval.max(std::time::Duration::from_secs(5))).await;
                continue;
            }
            let rejected = AtomicBool::new(false);
            match state.ready_index_jobs().await {
                Ok(jobs) => {
                    stream::iter(jobs)
                        .for_each_concurrent(4, |job| async {
                            if process_job(&state, job).await == JobOutcome::CredentialsRejected {
                                rejected.store(true, Ordering::Release);
                            }
                        })
                        .await;
                }
                Err(error) => tracing::error!("load message index outbox failed: {error}"),
            }
            if rejected.load(Ordering::Acquire) {
                // Queued jobs stay in the outbox and resume after a restart with a fixed key.
                tracing::error!(
                    "embedding or vector store rejected the configured credentials; automatic \
                     indexing stopped until the server restarts"
                );
                return;
            }
            tokio::time::sleep(interval).await;
        }
    });
}

#[derive(PartialEq, Eq)]
enum JobOutcome {
    Done,
    Retry,
    CredentialsRejected,
}

async fn process_job(state: &SharedState, job: IndexJob) -> JobOutcome {
    if let Err(error) = apply_job(state, &job).await {
        let outcome = if rejects_credentials(&error) {
            JobOutcome::CredentialsRejected
        } else {
            tracing::warn!(message_id = %job.message_id, "message indexing failed: {error:#}");
            JobOutcome::Retry
        };
        if let Err(store_error) = state.retry_index_job(&job, &error.to_string()).await {
            tracing::error!("record message index retry failed: {store_error}");
        }
        return outcome;
    }
    if let Err(error) = state.complete_index_job(&job).await {
        tracing::error!("complete message index job failed: {error}");
    }
    JobOutcome::Done
}

/// TG-1207: a provider that rejects the credentials (HTTP 401/403) will keep rejecting them.
/// Backing off per job still retried every job forever and every new message added another,
/// so the worker stops instead and says so once.
fn rejects_credentials(error: &anyhow::Error) -> bool {
    error
        .chain()
        .filter_map(|cause| cause.downcast_ref::<reqwest::Error>())
        .any(|cause| {
            matches!(
                cause.status(),
                Some(reqwest::StatusCode::UNAUTHORIZED | reqwest::StatusCode::FORBIDDEN)
            )
        })
}

async fn apply_job(state: &SharedState, job: &IndexJob) -> Result<()> {
    let index = state
        .message_index()
        .ok_or_else(|| anyhow::anyhow!("message index is disabled"))?;
    let message = state.indexed_message(job.message_id).await?;
    if job.operation == "delete" || message.is_none() {
        return index.clients.delete(job.message_id).await;
    }
    let message = message.expect("message checked above");
    let vector = index.clients.embed(&message.content).await?;
    index
        .clients
        .upsert(message.id, message.room_id, vector)
        .await
}
