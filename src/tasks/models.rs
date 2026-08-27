use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct RoomTaskSource {
    pub message_id: Uuid,
    pub sender: String,
    pub excerpt: String,
    pub recalled: bool,
    pub sent_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct RoomTask {
    pub id: Uuid,
    pub room_id: Uuid,
    pub title: String,
    pub status: String,
    pub assignee_id: Option<Uuid>,
    pub assignee_name: String,
    pub assignee_active: bool,
    pub created_by_id: Option<Uuid>,
    pub created_by_name: String,
    pub source: Option<RoomTaskSource>,
    pub due_at: Option<DateTime<Utc>>,
    pub version: i64,
    pub can_update: bool,
    pub can_delete: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateRoomTaskRequest {
    pub title: String,
    pub assignee_id: Option<Uuid>,
    pub due_at: Option<DateTime<Utc>>,
    pub source_message_id: Option<Uuid>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateRoomTaskRequest {
    pub title: String,
    pub status: String,
    pub assignee_id: Option<Uuid>,
    pub due_at: Option<DateTime<Utc>>,
    pub version: i64,
}

pub(super) fn valid_title(title: &str) -> bool {
    let length = title.trim().chars().count();
    (1..=120).contains(&length)
}

pub(super) fn valid_status(status: &str) -> bool {
    matches!(status, "open" | "in_progress" | "done" | "cancelled")
}

#[derive(Debug, PartialEq)]
pub(super) enum TaskMutation<T> {
    Applied(T),
    NotFound,
    Forbidden,
    Conflict,
    InvalidAssignee,
    InvalidSource,
    InvalidValue,
}
