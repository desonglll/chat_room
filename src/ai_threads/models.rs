use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct AiThread {
    pub id: Uuid,
    pub user_id: Uuid,
    pub title: String,
    pub room_id: Option<Uuid>,
    pub thinking_enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct AiThreadMessage {
    pub id: Uuid,
    pub thread_id: Uuid,
    pub role: String,
    pub content: String,
    pub room_id: Option<Uuid>,
    pub context_message_count: Option<i64>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Default, Deserialize, ToSchema)]
pub struct CreateAiThreadRequest {
    pub title: Option<String>,
    pub room_id: Option<Uuid>,
    pub thinking_enabled: Option<bool>,
}

#[derive(Debug, Default, Deserialize, ToSchema)]
pub struct UpdateAiThreadRequest {
    pub title: Option<String>,
    pub room_id: Option<Uuid>,
    #[serde(default)]
    pub clear_room: bool,
    pub thinking_enabled: Option<bool>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct QueryAiThreadRequest {
    pub question: String,
    pub room_id: Option<Uuid>,
}
