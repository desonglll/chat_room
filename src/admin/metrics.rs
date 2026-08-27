//! System administrator metrics, request telemetry, and retention maintenance.

use std::{
    collections::{BTreeMap, HashSet},
    sync::atomic::{AtomicU64, AtomicUsize, Ordering},
    time::Instant,
};

use axum::{
    extract::{Request, State},
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::Response,
    Json,
};
use chrono::{DateTime, Duration, Utc};
use serde::Serialize;
use sqlx::FromRow;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    admin::{
        access::require_admin,
        services::{collect_service_overview, ServiceOverview},
    },
    audit::AuditEventDraft,
    state::{with_pool, AppState, SharedState},
};

pub(crate) struct RuntimeMetrics {
    started_at: Instant,
    requests: AtomicU64,
    failures: AtomicU64,
    active: AtomicUsize,
    total_latency_micros: AtomicU64,
    max_latency_micros: AtomicU64,
}

impl Default for RuntimeMetrics {
    fn default() -> Self {
        Self {
            started_at: Instant::now(),
            requests: AtomicU64::new(0),
            failures: AtomicU64::new(0),
            active: AtomicUsize::new(0),
            total_latency_micros: AtomicU64::new(0),
            max_latency_micros: AtomicU64::new(0),
        }
    }
}

impl RuntimeMetrics {
    fn begin(&self) -> Instant {
        self.active.fetch_add(1, Ordering::Relaxed);
        Instant::now()
    }

    fn finish(&self, started: Instant, failed: bool) {
        let micros = started.elapsed().as_micros().min(u128::from(u64::MAX)) as u64;
        self.active.fetch_sub(1, Ordering::Relaxed);
        self.requests.fetch_add(1, Ordering::Relaxed);
        self.total_latency_micros
            .fetch_add(micros, Ordering::Relaxed);
        self.max_latency_micros.fetch_max(micros, Ordering::Relaxed);
        if failed {
            self.failures.fetch_add(1, Ordering::Relaxed);
        }
    }

    fn snapshot(&self) -> RuntimeSnapshot {
        let requests = self.requests.load(Ordering::Relaxed);
        let total = self.total_latency_micros.load(Ordering::Relaxed);
        RuntimeSnapshot {
            uptime_seconds: self.started_at.elapsed().as_secs(),
            requests,
            failures: self.failures.load(Ordering::Relaxed),
            active_requests: self.active.load(Ordering::Relaxed) as u64,
            average_latency_ms: if requests == 0 {
                0.0
            } else {
                total as f64 / requests as f64 / 1000.0
            },
            max_latency_ms: self.max_latency_micros.load(Ordering::Relaxed) as f64 / 1000.0,
        }
    }
}

pub async fn track_request(
    State(state): State<SharedState>,
    request: Request,
    next: Next,
) -> Response {
    let started = state.runtime_metrics().begin();
    let response = next.run(request).await;
    state
        .runtime_metrics()
        .finish(started, response.status().is_server_error());
    response
}

#[derive(Debug, Serialize, ToSchema)]
pub struct RuntimeSnapshot {
    uptime_seconds: u64,
    requests: u64,
    failures: u64,
    active_requests: u64,
    average_latency_ms: f64,
    max_latency_ms: f64,
}

#[derive(Debug, Serialize, ToSchema, FromRow)]
pub struct AdminTotals {
    users: i64,
    active_sessions: i64,
    active_rooms: i64,
    soft_deleted_rooms: i64,
    messages: i64,
    messages_24h: i64,
    attachments: i64,
    attachments_24h: i64,
    pending_uploads: i64,
}

#[derive(Debug, Serialize, ToSchema, FromRow)]
pub struct StorageMetrics {
    logical_bytes: i64,
    physical_bytes: i64,
    orphaned_attachments: i64,
    orphaned_bytes: i64,
    missing_hashes: i64,
}

#[derive(Debug, Serialize, ToSchema, FromRow)]
pub struct TopRoom {
    id: Uuid,
    name: String,
    messages: i64,
    active_members: i64,
    last_message_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AdminOverview {
    generated_at: DateTime<Utc>,
    database_backend: String,
    attachment_backend: String,
    online_users: u64,
    websocket_connections: u64,
    orphan_retention_hours: i64,
    deleted_room_retention_days: i64,
    chat_rooms_locked: bool,
    runtime: RuntimeSnapshot,
    auth_rate_limits: crate::security::AuthRateLimitSnapshot,
    totals: AdminTotals,
    storage: StorageMetrics,
    services: ServiceOverview,
    top_rooms: Vec<TopRoom>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PurgeResult {
    attachment_objects_deleted: u64,
    attachment_bytes_deleted: i64,
    rooms_deleted: u64,
}

#[derive(FromRow)]
struct OrphanCandidate {
    id: Uuid,
    storage_key: Option<String>,
    size_bytes: i64,
}

struct OrphanGroup {
    physical_key: String,
    query_key: String,
    storage_key: Option<String>,
    size_bytes: i64,
}

#[utoipa::path(
    get,
    path = "/api/admin/overview",
    responses(
        (status = 200, description = "System operations overview", body = AdminOverview),
        (status = 401, description = "Missing or expired session"),
        (status = 403, description = "Account is not a system administrator")
    )
)]
pub async fn overview(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> Result<Json<AdminOverview>, StatusCode> {
    require_admin(&state, &headers).await?;
    collect_overview(&state).await.map(Json).map_err(|error| {
        tracing::error!("load administrator overview failed: {error:#}");
        StatusCode::INTERNAL_SERVER_ERROR
    })
}

async fn collect_overview(state: &AppState) -> anyhow::Result<AdminOverview> {
    let now = Utc::now();
    let day_ago = now - Duration::hours(24);
    let totals: AdminTotals = with_pool!(state, |pool| {
        sqlx::query_as(
            "SELECT \
             (SELECT COUNT(*) FROM users) AS users, \
             (SELECT COUNT(*) FROM sessions WHERE expires_at > $1) AS active_sessions, \
             (SELECT COUNT(*) FROM rooms WHERE deleted_at IS NULL) AS active_rooms, \
             (SELECT COUNT(*) FROM rooms WHERE deleted_at IS NOT NULL) AS soft_deleted_rooms, \
             (SELECT COUNT(*) FROM messages) AS messages, \
             (SELECT COUNT(*) FROM messages WHERE created_at >= $2) AS messages_24h, \
             (SELECT COUNT(*) FROM attachments) AS attachments, \
             (SELECT COUNT(*) FROM attachments WHERE created_at >= $3) AS attachments_24h, \
             (SELECT COUNT(*) FROM attachment_uploads WHERE status = 'in_progress') \
               AS pending_uploads",
        )
        .bind(now)
        .bind(day_ago)
        .bind(day_ago)
        .fetch_one(pool)
        .await
    })?;
    let storage: StorageMetrics = with_pool!(state, |pool| {
        sqlx::query_as(
            "SELECT \
             CAST(COALESCE((SELECT SUM(size_bytes) FROM attachments), 0) AS BIGINT) \
               AS logical_bytes, \
             CAST(COALESCE((SELECT SUM(object_size) FROM (\
               SELECT MAX(size_bytes) AS object_size FROM attachments \
                 WHERE storage_key IS NOT NULL GROUP BY storage_key \
               UNION ALL SELECT size_bytes AS object_size FROM attachments \
                 WHERE storage_key IS NULL\
             ) objects), 0) AS BIGINT) AS physical_bytes, \
             (SELECT COUNT(*) FROM attachments WHERE orphaned_at IS NOT NULL) \
               AS orphaned_attachments, \
             CAST(COALESCE((SELECT SUM(size_bytes) FROM attachments \
               WHERE orphaned_at IS NOT NULL), 0) AS BIGINT) AS orphaned_bytes, \
             (SELECT COUNT(*) FROM attachments WHERE content_hash IS NULL) AS missing_hashes",
        )
        .fetch_one(pool)
        .await
    })?;
    let top_rooms: Vec<TopRoom> = with_pool!(state, |pool| {
        sqlx::query_as(
            "SELECT rooms.id, rooms.name, \
             (SELECT COUNT(*) FROM messages WHERE messages.room_id = rooms.id) AS messages, \
             (SELECT COUNT(*) FROM room_memberships WHERE room_id = rooms.id AND status = 'active') \
               AS active_members, \
             (SELECT MAX(created_at) FROM messages WHERE messages.room_id = rooms.id) \
               AS last_message_at \
             FROM rooms WHERE deleted_at IS NULL ORDER BY messages DESC, rooms.created_at DESC LIMIT 8",
        )
        .fetch_all(pool)
        .await
    })?;
    let (online_users, websocket_connections) = state.online_counts().await;
    let chat_rooms_locked = state.chat_rooms_locked().await?;
    let services = collect_service_overview(state).await;
    Ok(AdminOverview {
        generated_at: now,
        database_backend: state.database_backend().to_string(),
        attachment_backend: if state.attachment_store().oss_enabled() {
            "oss".into()
        } else {
            "local".into()
        },
        online_users,
        websocket_connections,
        orphan_retention_hours: state.orphan_retention_hours(),
        deleted_room_retention_days: state.deleted_room_retention_days(),
        chat_rooms_locked,
        runtime: state.runtime_metrics().snapshot(),
        auth_rate_limits: state.auth_rate_limits().snapshot(),
        totals,
        storage,
        services,
        top_rooms,
    })
}

#[utoipa::path(
    post,
    path = "/api/admin/maintenance/purge",
    responses(
        (status = 200, description = "Retention cleanup result", body = PurgeResult),
        (status = 401, description = "Missing or expired session"),
        (status = 403, description = "Account is not a system administrator")
    )
)]
pub async fn purge(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> Result<Json<PurgeResult>, StatusCode> {
    let actor = require_admin(&state, &headers).await?;
    state
        .record_audit_event(
            AuditEventDraft::system(&actor, "retention.purge_requested")
                .target_type("retained_data"),
        )
        .await
        .map_err(|error| {
            tracing::error!("required retention cleanup audit failed: {error}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    purge_retained_data(&state)
        .await
        .map(Json)
        .map_err(|error| {
            tracing::error!("administrator retention cleanup failed: {error:#}");
            StatusCode::INTERNAL_SERVER_ERROR
        })
}

async fn purge_retained_data(state: &AppState) -> anyhow::Result<PurgeResult> {
    let orphan_cutoff = Utc::now() - Duration::hours(state.orphan_retention_hours());
    let candidates: Vec<OrphanCandidate> = with_pool!(state, |pool| {
        sqlx::query_as(
            "SELECT a.id, a.storage_key, a.size_bytes FROM attachments a \
             WHERE a.orphaned_at IS NOT NULL AND a.orphaned_at <= $1 \
             AND NOT EXISTS (SELECT 1 FROM attachments newer \
               WHERE COALESCE(newer.storage_key, CAST(newer.id AS TEXT)) = \
                     COALESCE(a.storage_key, CAST(a.id AS TEXT)) \
               AND (newer.orphaned_at IS NULL OR newer.orphaned_at > $2)) \
             AND NOT EXISTS (SELECT 1 FROM attachments shared \
               JOIN messages ON messages.attachment_id = shared.id \
               WHERE COALESCE(shared.storage_key, CAST(shared.id AS TEXT)) = \
                     COALESCE(a.storage_key, CAST(a.id AS TEXT)) \
               AND messages.recalled_at IS NULL) \
             AND NOT EXISTS (SELECT 1 FROM attachments shared \
               JOIN favorites ON favorites.attachment_id = shared.id \
               WHERE COALESCE(shared.storage_key, CAST(shared.id AS TEXT)) = \
                     COALESCE(a.storage_key, CAST(a.id AS TEXT)))",
        )
        .bind(orphan_cutoff)
        .bind(orphan_cutoff)
        .fetch_all(pool)
        .await
    })?;
    let mut groups = BTreeMap::new();
    for candidate in candidates {
        let query_key = candidate
            .storage_key
            .clone()
            .unwrap_or_else(|| candidate.id.to_string());
        groups
            .entry(query_key.clone())
            .or_insert_with(|| OrphanGroup {
                physical_key: candidate
                    .storage_key
                    .clone()
                    .unwrap_or_else(|| candidate.id.simple().to_string()),
                query_key,
                storage_key: candidate.storage_key,
                size_bytes: candidate.size_bytes,
            });
    }

    let mut deleted_objects = 0;
    let mut deleted_bytes = 0;
    for group in groups.into_values() {
        let _guard = state.content_hash_locks().lock(&group.physical_key).await;
        let still_referenced: bool = with_pool!(state, |pool| {
            sqlx::query_scalar(
                "SELECT EXISTS(\
                   SELECT 1 FROM attachments a JOIN messages m ON m.attachment_id = a.id \
                   WHERE COALESCE(a.storage_key, CAST(a.id AS TEXT)) = $1 AND m.recalled_at IS NULL \
                   UNION ALL \
                   SELECT 1 FROM attachments a JOIN favorites f ON f.attachment_id = a.id \
                   WHERE COALESCE(a.storage_key, CAST(a.id AS TEXT)) = $1)",
            )
            .bind(&group.query_key)
            .fetch_one(pool)
            .await
        })?;
        if still_referenced {
            continue;
        }
        state.attachment_store().remove(&group.physical_key).await?;
        with_pool!(state, |pool| {
            match &group.storage_key {
                Some(storage_key) => sqlx::query(
                    "DELETE FROM attachments WHERE storage_key = $1 AND orphaned_at <= $2",
                )
                .bind(storage_key)
                .bind(orphan_cutoff)
                .execute(pool)
                .await
                .map(|_| ()),
                None => sqlx::query("DELETE FROM attachments WHERE id = $1 AND orphaned_at <= $2")
                    .bind(Uuid::parse_str(&group.query_key).expect("legacy attachment UUID"))
                    .bind(orphan_cutoff)
                    .execute(pool)
                    .await
                    .map(|_| ()),
            }
        })?;
        deleted_objects += 1;
        deleted_bytes += group.size_bytes;
    }

    let rooms_deleted = purge_deleted_rooms(state).await?;
    Ok(PurgeResult {
        attachment_objects_deleted: deleted_objects,
        attachment_bytes_deleted: deleted_bytes,
        rooms_deleted,
    })
}

async fn purge_deleted_rooms(state: &AppState) -> anyhow::Result<u64> {
    let cutoff = Utc::now() - Duration::days(state.deleted_room_retention_days());
    let room_ids: Vec<Uuid> = with_pool!(state, |pool| {
        sqlx::query_scalar(
            "SELECT id FROM rooms WHERE deleted_at IS NOT NULL AND deleted_at <= $1 \
             AND NOT EXISTS (SELECT 1 FROM attachments \
               JOIN favorites ON favorites.attachment_id = attachments.id \
               WHERE attachments.room_id = rooms.id)",
        )
        .bind(cutoff)
        .fetch_all(pool)
        .await
    })?;
    let mut deleted = 0;
    for room_id in room_ids {
        let objects: Vec<(Uuid, Option<String>)> = with_pool!(state, |pool| {
            sqlx::query_as("SELECT id, storage_key FROM attachments WHERE room_id = $1")
                .bind(room_id)
                .fetch_all(pool)
                .await
        })?;
        let changed = with_pool!(state, |pool| {
            sqlx::query("DELETE FROM rooms WHERE id = $1 AND deleted_at <= $2")
                .bind(room_id)
                .bind(cutoff)
                .execute(pool)
                .await
                .map(|result| result.rows_affected())
        })?;
        if changed == 0 {
            continue;
        }
        deleted += 1;
        let mut keys = HashSet::new();
        for (attachment_id, storage_key) in objects {
            let key = storage_key.unwrap_or_else(|| attachment_id.simple().to_string());
            if !keys.insert(key.clone()) {
                continue;
            }
            let still_used: bool = with_pool!(state, |pool| {
                sqlx::query_scalar(
                    "SELECT EXISTS(SELECT 1 FROM attachments WHERE storage_key = $1)",
                )
                .bind(&key)
                .fetch_one(pool)
                .await
            })?;
            if !still_used {
                let _guard = state.content_hash_locks().lock(&key).await;
                state.attachment_store().remove(&key).await?;
            }
        }
    }
    Ok(deleted)
}
