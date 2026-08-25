//! Read-only dependency health and vector-search diagnostics for system admins.

use std::time::{Duration, Instant};

use axum::{extract::State, http::HeaderMap, http::StatusCode, Json};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use super::metrics::require_admin;
use crate::state::{with_pool, AppState, SharedState};

const VECTOR_PROBE_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Serialize, ToSchema)]
pub struct ServiceStatus {
    id: String,
    label: String,
    state: String,
    latency_ms: Option<u64>,
    detail: String,
}

#[derive(Debug, Default, Serialize, ToSchema)]
pub struct VectorIndexStatus {
    points: Option<u64>,
    pending_jobs: i64,
    retrying_jobs: i64,
    last_error: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ServiceOverview {
    items: Vec<ServiceStatus>,
    vector_index: VectorIndexStatus,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct VectorProbeRequest {
    room_id: Uuid,
    query: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct VectorProbeMatch {
    message_id: Uuid,
    score: f64,
    sender: String,
    content: String,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct VectorProbeResult {
    latency_ms: u64,
    matches: Vec<VectorProbeMatch>,
}

pub(crate) async fn collect_service_overview(state: &AppState) -> ServiceOverview {
    let database = probe_database(state).await;
    let redis = probe_redis(state).await;
    let (vector_store, points) = probe_vector_store(state).await;
    let embedding = configured_service(
        "embedding",
        "Embedding",
        state.config.vector_store.enabled,
        state.config.vector_store.embedding_model.clone(),
    );
    let ai_provider = ai_service(state).await;
    let mut vector_index = index_queue_status(state).await;
    vector_index.points = points;
    ServiceOverview {
        items: vec![database, redis, vector_store, embedding, ai_provider],
        vector_index,
    }
}

async fn probe_database(state: &AppState) -> ServiceStatus {
    let started = Instant::now();
    let result = with_pool!(state, |pool| {
        sqlx::query_scalar::<_, i64>("SELECT CAST(1 AS BIGINT)")
            .fetch_one(pool)
            .await
    });
    status_from_result(
        "database",
        if state.database_backend() == "postgres" {
            "PostgreSQL"
        } else {
            "SQLite"
        },
        started,
        result.map(|_| ()),
    )
}

async fn probe_redis(state: &AppState) -> ServiceStatus {
    if !state.config.redis.enabled {
        return disabled("redis", "Redis");
    }
    let Some(cache) = state.redis_cache.as_ref() else {
        return degraded("redis", "Redis", None, "连接失败，当前回退到数据库读取");
    };
    let started = Instant::now();
    status_from_result("redis", "Redis", started, cache.ping().await)
}

async fn probe_vector_store(state: &AppState) -> (ServiceStatus, Option<u64>) {
    if !state.config.vector_store.enabled {
        return (disabled("vector_store", "Qdrant"), None);
    }
    let Some(index) = state.message_index.as_ref() else {
        return (
            degraded("vector_store", "Qdrant", None, "向量索引初始化失败"),
            None,
        );
    };
    let started = Instant::now();
    match tokio::time::timeout(Duration::from_secs(2), index.point_count()).await {
        Ok(Ok(points)) => (
            healthy(
                "vector_store",
                "Qdrant",
                started.elapsed(),
                format!("{points} 个向量点"),
            ),
            Some(points),
        ),
        Ok(Err(error)) => (
            degraded(
                "vector_store",
                "Qdrant",
                Some(started.elapsed()),
                concise_error(&error),
            ),
            None,
        ),
        Err(_) => (
            degraded(
                "vector_store",
                "Qdrant",
                Some(started.elapsed()),
                "探测超时",
            ),
            None,
        ),
    }
}

async fn index_queue_status(state: &AppState) -> VectorIndexStatus {
    let result: Result<(i64, i64, Option<String>), sqlx::Error> = with_pool!(state, |pool| {
        sqlx::query_as(
            "SELECT COUNT(*) AS pending_jobs, \
             COUNT(CASE WHEN attempt_count > 0 THEN 1 END) AS retrying_jobs, \
             MAX(last_error) AS last_error FROM message_index_outbox",
        )
        .fetch_one(pool)
        .await
    });
    match result {
        Ok((pending_jobs, retrying_jobs, last_error)) => VectorIndexStatus {
            points: None,
            pending_jobs,
            retrying_jobs,
            last_error,
        },
        Err(error) => VectorIndexStatus {
            last_error: Some(concise_error(&error)),
            ..VectorIndexStatus::default()
        },
    }
}

#[utoipa::path(
    post,
    path = "/api/admin/vector/probe",
    request_body = VectorProbeRequest,
    responses(
        (status = 200, description = "Semantic message retrieval diagnostic", body = VectorProbeResult),
        (status = 400, description = "Invalid query"),
        (status = 403, description = "Admin is not an active member of the room"),
        (status = 409, description = "Vector search is disabled"),
        (status = 504, description = "Vector search timed out")
    )
)]
pub async fn probe_vector_search(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(payload): Json<VectorProbeRequest>,
) -> Result<Json<VectorProbeResult>, StatusCode> {
    let user = require_admin(&state, &headers).await?;
    let query = payload.query.trim();
    if query.is_empty() || query.chars().count() > 500 {
        return Err(StatusCode::BAD_REQUEST);
    }
    if !state
        .has_room_permission(payload.room_id, user.id, "message.send")
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    {
        return Err(StatusCode::FORBIDDEN);
    }
    let index = state.message_index().ok_or(StatusCode::CONFLICT)?;
    let started = Instant::now();
    let candidates = tokio::time::timeout(
        VECTOR_PROBE_TIMEOUT,
        index.related_messages(payload.room_id, query),
    )
    .await
    .map_err(|_| StatusCode::GATEWAY_TIMEOUT)?
    .map_err(|error| {
        tracing::warn!("vector search probe failed: {error:#}");
        StatusCode::BAD_GATEWAY
    })?;
    let scores: std::collections::HashMap<Uuid, f64> = candidates
        .iter()
        .map(|candidate| (candidate.id, candidate.score))
        .collect();
    let ids: Vec<Uuid> = candidates
        .into_iter()
        .map(|candidate| candidate.id)
        .collect();
    let matches = state
        .authorized_retrieved_messages(user.id, payload.room_id, &ids)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .into_iter()
        .take(index.result_limit())
        .map(|message| VectorProbeMatch {
            message_id: message.id,
            score: scores.get(&message.id).copied().unwrap_or_default(),
            sender: message.sender,
            content: message.content,
            created_at: message.created_at,
        })
        .collect();
    Ok(Json(VectorProbeResult {
        latency_ms: duration_ms(started.elapsed()),
        matches,
    }))
}

fn configured_service(id: &str, label: &str, enabled: bool, detail: String) -> ServiceStatus {
    if enabled {
        ServiceStatus {
            id: id.into(),
            label: label.into(),
            state: "configured".into(),
            latency_ms: None,
            detail,
        }
    } else {
        disabled(id, label)
    }
}

async fn ai_service(state: &AppState) -> ServiceStatus {
    let Ok(options) = state.ai_model_options().await else {
        return degraded("ai_provider", "LLM", None, "读取模型配置失败");
    };
    let enabled = options.iter().filter(|option| option.enabled).count();
    let ready = options
        .iter()
        .filter(|option| option.enabled && option.ready)
        .count();
    if enabled == 0 {
        return disabled("ai_provider", "LLM");
    }
    if ready == 0 {
        return degraded(
            "ai_provider",
            "LLM",
            None,
            "已配置模型，但全部缺少 API 凭据",
        );
    }
    configured_service(
        "ai_provider",
        "LLM",
        true,
        format!("{ready}/{enabled} 个模型可用"),
    )
}

fn status_from_result(
    id: &str,
    label: &str,
    started: Instant,
    result: Result<(), impl std::fmt::Display>,
) -> ServiceStatus {
    match result {
        Ok(()) => healthy(id, label, started.elapsed(), "连接正常"),
        Err(error) => degraded(id, label, Some(started.elapsed()), error.to_string()),
    }
}

fn healthy(id: &str, label: &str, elapsed: Duration, detail: impl Into<String>) -> ServiceStatus {
    ServiceStatus {
        id: id.into(),
        label: label.into(),
        state: "healthy".into(),
        latency_ms: Some(duration_ms(elapsed)),
        detail: detail.into(),
    }
}

fn degraded(
    id: &str,
    label: &str,
    elapsed: Option<Duration>,
    detail: impl Into<String>,
) -> ServiceStatus {
    ServiceStatus {
        id: id.into(),
        label: label.into(),
        state: "degraded".into(),
        latency_ms: elapsed.map(duration_ms),
        detail: detail.into(),
    }
}

fn disabled(id: &str, label: &str) -> ServiceStatus {
    ServiceStatus {
        id: id.into(),
        label: label.into(),
        state: "disabled".into(),
        latency_ms: None,
        detail: "未启用".into(),
    }
}

fn duration_ms(duration: Duration) -> u64 {
    duration.as_millis().min(u128::from(u64::MAX)) as u64
}

fn concise_error(error: &impl std::fmt::Display) -> String {
    error.to_string().chars().take(160).collect()
}
