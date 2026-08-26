//! Administrator-triggered reconciliation of rebuildable derived indexes.

use axum::{extract::State, http::HeaderMap, http::StatusCode, routing::post, Json, Router};
use serde::{Deserialize, Serialize};

use super::metrics::require_admin;
use crate::state::{with_pool, AppState, SharedState};

const VECTOR_SYNC_SQL: &str = "INSERT INTO message_index_outbox \
    (message_id, operation, attempt_count, generation, next_attempt_at, last_error, updated_at) \
    SELECT messages.id, CASE WHEN messages.recalled_at IS NULL \
        AND trim(messages.content) <> '' AND rooms.deleted_at IS NULL \
        THEN 'upsert' ELSE 'delete' END, 0, 1, CURRENT_TIMESTAMP, NULL, CURRENT_TIMESTAMP \
    FROM messages JOIN rooms ON rooms.id = messages.room_id \
    ON CONFLICT (message_id) DO UPDATE SET operation = excluded.operation, attempt_count = 0, \
        generation = message_index_outbox.generation + 1, next_attempt_at = CURRENT_TIMESTAMP, \
        last_error = NULL, updated_at = CURRENT_TIMESTAMP";

const GRAPH_SYNC_SQL: &str = "INSERT INTO message_graph_outbox \
    (message_id, room_id, operation, attempt_count, generation, next_attempt_at, last_error, updated_at) \
    SELECT messages.id, messages.room_id, CASE WHEN messages.recalled_at IS NULL \
        AND trim(messages.content) <> '' AND rooms.deleted_at IS NULL \
        THEN 'upsert' ELSE 'delete' END, 0, 1, CURRENT_TIMESTAMP, NULL, CURRENT_TIMESTAMP \
    FROM messages JOIN rooms ON rooms.id = messages.room_id \
    ON CONFLICT (message_id) DO UPDATE SET room_id = excluded.room_id, \
        operation = excluded.operation, attempt_count = 0, \
        generation = message_graph_outbox.generation + 1, next_attempt_at = CURRENT_TIMESTAMP, \
        last_error = NULL, updated_at = CURRENT_TIMESTAMP";

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IndexSyncTarget {
    Vector,
    Graph,
}

#[derive(Debug, Deserialize)]
pub struct IndexSyncRequest {
    target: IndexSyncTarget,
}

#[derive(Debug, Serialize)]
pub struct IndexSyncResult {
    target: IndexSyncTarget,
    queued_messages: u64,
}

#[derive(Debug, Default)]
pub(crate) struct EnabledIndexSyncResult {
    pub vector_messages: u64,
    pub graph_messages: u64,
}

pub fn routes() -> Router<SharedState> {
    Router::new().route("/api/admin/indexes/sync", post(sync))
}

async fn sync(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(request): Json<IndexSyncRequest>,
) -> Result<Json<IndexSyncResult>, StatusCode> {
    require_admin(&state, &headers).await?;
    if !target_enabled(&state, request.target) {
        return Err(StatusCode::CONFLICT);
    }
    let queued_messages = queue_messages(&state, request.target)
        .await
        .map_err(|error| {
            tracing::error!(target = ?request.target, "queue index synchronization failed: {error}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(Json(IndexSyncResult {
        target: request.target,
        queued_messages,
    }))
}

pub(crate) async fn sync_enabled(state: &AppState) -> Result<EnabledIndexSyncResult, sqlx::Error> {
    let vector_messages = if state.config.vector_store.enabled {
        queue_messages(state, IndexSyncTarget::Vector).await?
    } else {
        0
    };
    let graph_messages = if state.config.knowledge_graph.enabled {
        queue_messages(state, IndexSyncTarget::Graph).await?
    } else {
        0
    };
    Ok(EnabledIndexSyncResult {
        vector_messages,
        graph_messages,
    })
}

fn target_enabled(state: &AppState, target: IndexSyncTarget) -> bool {
    match target {
        IndexSyncTarget::Vector => state.config.vector_store.enabled,
        IndexSyncTarget::Graph => state.config.knowledge_graph.enabled,
    }
}

async fn queue_messages(state: &AppState, target: IndexSyncTarget) -> Result<u64, sqlx::Error> {
    let sql = match target {
        IndexSyncTarget::Vector => VECTOR_SYNC_SQL,
        IndexSyncTarget::Graph => GRAPH_SYNC_SQL,
    };
    with_pool!(state, |pool| {
        sqlx::query(sql)
            .execute(pool)
            .await
            .map(|result| result.rows_affected())
    })
}
