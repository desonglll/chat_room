//! Administrator-triggered reconciliation of rebuildable derived indexes.

use axum::{extract::State, http::HeaderMap, http::StatusCode, routing::post, Json, Router};
use serde::{Deserialize, Serialize};

use super::access::require_admin;
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

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IndexSyncTarget {
    Vector,
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
    if !state.config.vector_store.enabled {
        return Err(StatusCode::CONFLICT);
    }
    let queued_messages = queue_messages(&state).await.map_err(|error| {
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
        queue_messages(state).await?
    } else {
        0
    };
    Ok(EnabledIndexSyncResult { vector_messages })
}

async fn queue_messages(state: &AppState) -> Result<u64, sqlx::Error> {
    with_pool!(state, |pool| {
        sqlx::query(VECTOR_SYNC_SQL)
            .execute(pool)
            .await
            .map(|result| result.rows_affected())
    })
}
