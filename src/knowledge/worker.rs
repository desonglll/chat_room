use std::sync::atomic::Ordering;

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
            match state.ready_index_jobs().await {
                Ok(jobs) => {
                    stream::iter(jobs)
                        .for_each_concurrent(4, |job| process_job(&state, job))
                        .await;
                }
                Err(error) => tracing::error!("load message index outbox failed: {error}"),
            }
            tokio::time::sleep(interval).await;
        }
    });
}

async fn process_job(state: &SharedState, job: IndexJob) {
    if let Err(error) = apply_job(state, &job).await {
        tracing::warn!(message_id = %job.message_id, "message indexing failed: {error:#}");
        if let Err(store_error) = state.retry_index_job(&job, &error.to_string()).await {
            tracing::error!("record message index retry failed: {store_error}");
        }
        return;
    }
    if let Err(error) = state.complete_index_job(&job).await {
        tracing::error!("complete message index job failed: {error}");
    }
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
