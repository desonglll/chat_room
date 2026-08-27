//! Public health probes, low-cardinality metrics, and request correlation.

use std::time::Instant;

use axum::{
    extract::{MatchedPath, Request, State},
    http::{header::CONTENT_TYPE, HeaderValue, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use serde::Serialize;
use tracing::Instrument;
use uuid::Uuid;

use crate::{admin::services::collect_service_overview, state::SharedState};

const REQUEST_ID_HEADER: &str = "x-request-id";

#[derive(Serialize)]
struct LiveStatus {
    status: &'static str,
}

#[derive(Serialize)]
struct DependencyStatus {
    id: String,
    status: String,
    required: bool,
}

#[derive(Serialize)]
struct ReadyStatus {
    status: &'static str,
    dependencies: Vec<DependencyStatus>,
}

pub fn routes() -> Router<SharedState> {
    Router::new()
        .route("/health/live", get(live))
        .route("/health/ready", get(ready))
        .route("/metrics", get(metrics))
}

async fn live() -> Json<LiveStatus> {
    Json(LiveStatus { status: "live" })
}

async fn ready(State(state): State<SharedState>) -> Response {
    let (status, snapshot) = readiness(&state).await;
    (status, Json(snapshot)).into_response()
}

async fn readiness(state: &SharedState) -> (StatusCode, ReadyStatus) {
    let overview = collect_service_overview(state).await;
    let dependencies = overview
        .items
        .into_iter()
        .filter(|service| service.id != "embedding")
        .map(|service| DependencyStatus {
            required: service.id == "database"
                || state.config.observability.is_required(&service.id),
            id: service.id,
            status: service.state,
        })
        .collect::<Vec<_>>();
    let unavailable_required = dependencies.iter().any(|dependency| {
        dependency.required && !matches!(dependency.status.as_str(), "healthy" | "configured")
    });
    let degraded_optional = dependencies
        .iter()
        .any(|dependency| !dependency.required && dependency.status == "degraded");
    let status = if unavailable_required {
        "not_ready"
    } else if degraded_optional {
        "degraded"
    } else {
        "ready"
    };
    (
        if unavailable_required {
            StatusCode::SERVICE_UNAVAILABLE
        } else {
            StatusCode::OK
        },
        ReadyStatus {
            status,
            dependencies,
        },
    )
}

async fn metrics(State(state): State<SharedState>) -> impl IntoResponse {
    let (ready_status, readiness) = readiness(&state).await;
    let runtime = state.runtime_metrics().snapshot();
    let (online_users, websocket_connections) = state.online_counts().await;
    let duration_sum = runtime.average_latency_ms * runtime.requests as f64 / 1000.0;
    let mut body = format!(
        "# TYPE chat_room_http_requests_total counter\nchat_room_http_requests_total {}\n\
         # TYPE chat_room_http_request_failures_total counter\nchat_room_http_request_failures_total {}\n\
         # TYPE chat_room_http_active_requests gauge\nchat_room_http_active_requests {}\n\
         # TYPE chat_room_http_request_duration_seconds summary\n\
         chat_room_http_request_duration_seconds_sum {duration_sum}\n\
         chat_room_http_request_duration_seconds_count {}\n\
         # TYPE chat_room_http_request_duration_seconds_max gauge\n\
         chat_room_http_request_duration_seconds_max {}\n\
         # TYPE chat_room_uptime_seconds gauge\nchat_room_uptime_seconds {}\n\
         # TYPE chat_room_online_users gauge\nchat_room_online_users {online_users}\n\
         # TYPE chat_room_websocket_connections gauge\nchat_room_websocket_connections {websocket_connections}\n\
         # TYPE chat_room_ready gauge\nchat_room_ready {}\n\
         # TYPE chat_room_dependency_up gauge\n",
        runtime.requests,
        runtime.failures,
        runtime.active_requests,
        runtime.requests,
        runtime.max_latency_ms / 1000.0,
        runtime.uptime_seconds,
        u8::from(ready_status == StatusCode::OK),
    );
    for dependency in readiness.dependencies {
        let up = matches!(dependency.status.as_str(), "healthy" | "configured");
        body.push_str(&format!(
            "chat_room_dependency_up{{dependency=\"{}\",required=\"{}\"}} {}\n",
            dependency.id,
            dependency.required,
            u8::from(up)
        ));
    }
    (
        [(CONTENT_TYPE, "text/plain; version=0.0.4; charset=utf-8")],
        body,
    )
}

pub async fn request_context(mut request: Request, next: Next) -> Response {
    let request_id = incoming_request_id(&request).unwrap_or_else(|| Uuid::new_v4().to_string());
    let method = request.method().as_str().to_owned();
    let route = request
        .extensions()
        .get::<MatchedPath>()
        .map(MatchedPath::as_str)
        .unwrap_or("unmatched")
        .to_owned();
    request.extensions_mut().insert(request_id.clone());
    let span = tracing::info_span!("http.request", request_id = %request_id, %method, %route);
    async move {
        let started = Instant::now();
        let mut response = next.run(request).await;
        let status = response.status().as_u16();
        let latency_ms = started.elapsed().as_millis() as u64;
        tracing::info!(status, latency_ms, "request completed");
        if let Ok(value) = HeaderValue::from_str(&request_id) {
            response.headers_mut().insert(REQUEST_ID_HEADER, value);
        }
        response
    }
    .instrument(span)
    .await
}

fn incoming_request_id(request: &Request) -> Option<String> {
    request
        .headers()
        .get(REQUEST_ID_HEADER)?
        .to_str()
        .ok()
        .filter(|value| {
            !value.is_empty()
                && value.len() <= 64
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"-_.:".contains(&byte))
        })
        .map(str::to_owned)
}
