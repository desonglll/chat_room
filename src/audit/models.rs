use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::models::User;

#[derive(Debug, Serialize, ToSchema)]
pub struct AuditEvent {
    pub id: Uuid,
    pub scope: String,
    pub room_id: Option<Uuid>,
    pub actor_user_id: Uuid,
    pub actor_username: String,
    pub event_type: String,
    pub target_type: Option<String>,
    pub target_id: Option<String>,
    pub details: BTreeMap<String, String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AuditEventPage {
    pub items: Vec<AuditEvent>,
    pub next_cursor: Option<String>,
}

pub struct AuditEventDraft {
    pub(crate) scope: &'static str,
    pub(crate) room_id: Option<Uuid>,
    pub(crate) actor_user_id: Uuid,
    pub(crate) actor_username: String,
    pub(crate) event_type: &'static str,
    pub(crate) target_type: Option<&'static str>,
    pub(crate) target_id: Option<String>,
    pub(crate) details: BTreeMap<String, String>,
}

impl AuditEventDraft {
    pub fn system(actor: &User, event_type: &'static str) -> Self {
        Self::new("system", None, actor, event_type)
    }

    pub fn room(actor: &User, room_id: Uuid, event_type: &'static str) -> Self {
        Self::new("room", Some(room_id), actor, event_type)
    }

    fn new(
        scope: &'static str,
        room_id: Option<Uuid>,
        actor: &User,
        event_type: &'static str,
    ) -> Self {
        Self {
            scope,
            room_id,
            actor_user_id: actor.id,
            actor_username: actor.username.clone(),
            event_type,
            target_type: None,
            target_id: None,
            details: BTreeMap::new(),
        }
    }

    pub fn target(mut self, target_type: &'static str, target_id: impl ToString) -> Self {
        self.target_type = Some(target_type);
        self.target_id = Some(target_id.to_string());
        self
    }

    pub fn target_type(mut self, target_type: &'static str) -> Self {
        self.target_type = Some(target_type);
        self
    }

    pub fn detail(mut self, key: &'static str, value: impl ToString) -> Self {
        if !is_sensitive_key(key) && self.details.len() < 12 {
            let value = value.to_string().chars().take(120).collect();
            self.details.insert(key.into(), value);
        }
        self
    }
}

fn is_sensitive_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    [
        "token", "password", "secret", "content", "body", "prompt", "evidence", "message",
    ]
    .iter()
    .any(|blocked| key.contains(blocked))
}
