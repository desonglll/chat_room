use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::state::{with_pool, AppState};

use super::models::{AuditEvent, AuditEventDraft, AuditEventPage};

#[derive(sqlx::FromRow)]
struct AuditEventRow {
    id: Uuid,
    scope: String,
    room_id: Option<Uuid>,
    actor_user_id: Uuid,
    actor_username: String,
    event_type: String,
    target_type: Option<String>,
    target_id: Option<String>,
    details_json: String,
    created_at: DateTime<Utc>,
}

impl AuditEventRow {
    fn into_event(self) -> AuditEvent {
        AuditEvent {
            id: self.id,
            scope: self.scope,
            room_id: self.room_id,
            actor_user_id: self.actor_user_id,
            actor_username: self.actor_username,
            event_type: self.event_type,
            target_type: self.target_type,
            target_id: self.target_id,
            details: serde_json::from_str::<BTreeMap<String, String>>(&self.details_json)
                .unwrap_or_default(),
            created_at: self.created_at,
        }
    }
}

pub(crate) struct AuditFilter {
    pub actor: String,
    pub event_type: String,
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
    pub cursor_at: Option<DateTime<Utc>>,
    pub cursor_id: Option<Uuid>,
    pub limit: i64,
}

impl AppState {
    pub async fn record_audit_event(&self, draft: AuditEventDraft) -> Result<Uuid, sqlx::Error> {
        let id = Uuid::new_v4();
        let created_at = Utc::now();
        let details_json = serde_json::to_string(&draft.details).unwrap_or_else(|_| "{}".into());
        with_pool!(self, |pool| {
            sqlx::query(
                "INSERT INTO audit_events (id, scope, room_id, actor_user_id, actor_username, \
                 event_type, target_type, target_id, details_json, created_at) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
            )
            .bind(id)
            .bind(draft.scope)
            .bind(draft.room_id)
            .bind(draft.actor_user_id)
            .bind(draft.actor_username)
            .bind(draft.event_type)
            .bind(draft.target_type)
            .bind(draft.target_id)
            .bind(details_json)
            .bind(created_at)
            .execute(pool)
            .await
            .map(|_| ())
        })?;
        Ok(id)
    }

    pub(crate) async fn audit_events(
        &self,
        scope: &str,
        room_id: Option<Uuid>,
        filter: AuditFilter,
    ) -> Result<AuditEventPage, sqlx::Error> {
        let mut rows: Vec<AuditEventRow> = with_pool!(self, |pool| {
            sqlx::query_as(
                "SELECT id, scope, room_id, actor_user_id, actor_username, event_type, \
                 target_type, target_id, details_json, created_at FROM audit_events \
                 WHERE scope = $1 AND ($2 IS NULL OR room_id = $2) \
                 AND ($3 = '' OR LOWER(actor_username) LIKE '%' || LOWER($3) || '%') \
                 AND ($4 = '' OR event_type = $4) \
                 AND ($5 IS NULL OR created_at >= $5) AND ($6 IS NULL OR created_at <= $6) \
                 AND ($7 IS NULL OR created_at < $7 OR (created_at = $7 AND id < $8)) \
                 ORDER BY created_at DESC, id DESC LIMIT $9",
            )
            .bind(scope)
            .bind(room_id)
            .bind(filter.actor)
            .bind(filter.event_type)
            .bind(filter.from)
            .bind(filter.to)
            .bind(filter.cursor_at)
            .bind(filter.cursor_id)
            .bind(filter.limit + 1)
            .fetch_all(pool)
            .await
        })?;
        let has_next = rows.len() > filter.limit as usize;
        rows.truncate(filter.limit as usize);
        let next_cursor = has_next.then(|| {
            let last = rows.last().expect("a page with overflow has a last row");
            format!("{}|{}", last.created_at.to_rfc3339(), last.id)
        });
        Ok(AuditEventPage {
            items: rows.into_iter().map(AuditEventRow::into_event).collect(),
            next_cursor,
        })
    }
}
