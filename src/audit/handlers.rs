use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::get,
    Json, Router,
};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use uuid::Uuid;

use crate::{admin::access::require_admin, state::SharedState, user_handlers::bearer_token};

use super::{models::AuditEventPage, store::AuditFilter};

#[derive(Default, Deserialize)]
pub struct AuditEventQuery {
    #[serde(default)]
    actor: String,
    #[serde(default)]
    event_type: String,
    from: Option<String>,
    to: Option<String>,
    cursor: Option<String>,
    limit: Option<i64>,
}

pub fn routes() -> Router<SharedState> {
    Router::new()
        .route("/api/admin/audit-events", get(list_system))
        .route("/api/rooms/:id/audit-events", get(list_room))
}

/// List deployment-wide management audit events.
#[utoipa::path(
    get,
    path = "/api/admin/audit-events",
    responses(
        (status = 200, description = "System audit events", body = AuditEventPage),
        (status = 401, description = "Missing or expired session"),
        (status = 403, description = "Account is not a system administrator")
    )
)]
pub async fn list_system(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Query(query): Query<AuditEventQuery>,
) -> Result<Json<AuditEventPage>, StatusCode> {
    require_admin(&state, &headers).await?;
    list(&state, "system", None, query).await
}

/// List management audit events for one Room.
#[utoipa::path(
    get,
    path = "/api/rooms/{id}/audit-events",
    params(("id" = Uuid, Path, description = "Room identifier")),
    responses(
        (status = 200, description = "Room audit events", body = AuditEventPage),
        (status = 401, description = "Missing or expired session"),
        (status = 403, description = "Room management permission required"),
        (status = 404, description = "Room not found")
    )
)]
pub async fn list_room(
    State(state): State<SharedState>,
    Path(room_id): Path<Uuid>,
    headers: HeaderMap,
    Query(query): Query<AuditEventQuery>,
) -> Result<Json<AuditEventPage>, StatusCode> {
    let token = bearer_token(&headers)?;
    let user = state
        .session_user(token)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::UNAUTHORIZED)?;
    if state.room(room_id).await.is_none() {
        return Err(StatusCode::NOT_FOUND);
    }
    state
        .has_room_permission(room_id, user.id, "members.review")
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .then_some(())
        .ok_or(StatusCode::FORBIDDEN)?;
    list(&state, "room", Some(room_id), query).await
}

async fn list(
    state: &SharedState,
    scope: &str,
    room_id: Option<Uuid>,
    query: AuditEventQuery,
) -> Result<Json<AuditEventPage>, StatusCode> {
    let filter = normalize_query(query)?;
    state
        .audit_events(scope, room_id, filter)
        .await
        .map(Json)
        .map_err(|error| {
            tracing::error!("list audit events failed: {error}");
            StatusCode::INTERNAL_SERVER_ERROR
        })
}

fn normalize_query(query: AuditEventQuery) -> Result<AuditFilter, StatusCode> {
    let actor = query.actor.trim().to_owned();
    let event_type = query.event_type.trim().to_owned();
    if actor.chars().count() > 64
        || event_type.chars().count() > 80
        || actor.chars().any(char::is_control)
        || event_type.chars().any(char::is_control)
    {
        return Err(StatusCode::BAD_REQUEST);
    }
    let from = parse_time(query.from)?;
    let to = parse_time(query.to)?;
    if from.zip(to).is_some_and(|(from, to)| from > to) {
        return Err(StatusCode::BAD_REQUEST);
    }
    let (cursor_at, cursor_id) = parse_cursor(query.cursor)?;
    Ok(AuditFilter {
        actor,
        event_type,
        from,
        to,
        cursor_at,
        cursor_id,
        limit: query.limit.unwrap_or(50).clamp(1, 100),
    })
}

fn parse_time(value: Option<String>) -> Result<Option<DateTime<Utc>>, StatusCode> {
    value
        .map(|value| {
            DateTime::parse_from_rfc3339(&value)
                .map(|value| value.with_timezone(&Utc))
                .map_err(|_| StatusCode::BAD_REQUEST)
        })
        .transpose()
}

fn parse_cursor(
    cursor: Option<String>,
) -> Result<(Option<DateTime<Utc>>, Option<Uuid>), StatusCode> {
    let Some(cursor) = cursor else {
        return Ok((None, None));
    };
    let (created_at, id) = cursor.rsplit_once('|').ok_or(StatusCode::BAD_REQUEST)?;
    let created_at = DateTime::parse_from_rfc3339(created_at)
        .map_err(|_| StatusCode::BAD_REQUEST)?
        .with_timezone(&Utc);
    let id = Uuid::parse_str(id).map_err(|_| StatusCode::BAD_REQUEST)?;
    Ok((Some(created_at), Some(id)))
}
