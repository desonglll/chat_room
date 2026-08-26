use std::collections::HashMap;
use std::sync::atomic::Ordering;

use anyhow::Result;
use futures_util::{stream, StreamExt};

use super::models::EpisodeUpsert;
use super::store::GraphJob;
use crate::state::SharedState;

pub fn ensure_worker(state: SharedState) {
    if state.knowledge_graph().is_none()
        || state
            .knowledge_graph_worker_started
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
    {
        return;
    }
    tokio::spawn(async move {
        let graph = state.knowledge_graph().expect("graph checked above");
        let interval = graph.worker_interval;
        let concurrency = graph.worker_concurrency;
        loop {
            match state.ready_graph_jobs().await {
                Ok(jobs) => process_ready_jobs(&state, jobs, concurrency).await,
                Err(error) => tracing::error!("load graph outbox failed: {error}"),
            }
            tokio::time::sleep(interval).await;
        }
    });
}

async fn process_ready_jobs(state: &SharedState, jobs: Vec<GraphJob>, concurrency: usize) {
    let mut room_jobs: HashMap<_, Vec<_>> = HashMap::new();
    for job in jobs {
        room_jobs.entry(job.room_id).or_default().push(job);
    }
    stream::iter(room_jobs.into_values())
        .for_each_concurrent(concurrency, |jobs| async move {
            for job in jobs {
                process_job(state, job).await;
            }
        })
        .await;
}

async fn process_job(state: &SharedState, job: GraphJob) {
    if let Err(error) = apply_job(state, &job).await {
        tracing::warn!(message_id = %job.message_id, room_id = %job.room_id,
            "knowledge graph indexing failed: {error:#}");
        if let Err(store_error) = state.retry_graph_job(&job, &error.to_string()).await {
            tracing::error!("record graph retry failed: {store_error}");
        }
        return;
    }
    if let Err(error) = state.complete_graph_job(&job).await {
        tracing::error!("complete graph job failed: {error}");
    }
}

async fn apply_job(state: &SharedState, job: &GraphJob) -> Result<()> {
    let graph = state
        .knowledge_graph()
        .ok_or_else(|| anyhow::anyhow!("knowledge graph is disabled"))?;
    let message = state.graph_message(job.message_id).await?;
    if job.operation == "delete" || message.is_none() {
        return graph.delete(job.room_id, job.message_id).await;
    }
    let message = message.expect("message checked above");
    if message.room_id != job.room_id || message.id != job.message_id {
        anyhow::bail!("graph job source identifiers changed");
    }
    graph
        .upsert(
            message.id,
            &EpisodeUpsert {
                room_id: message.room_id,
                sender: &message.sender,
                content: &message.content,
                created_at: message.created_at,
            },
        )
        .await
}
