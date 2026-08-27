use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::types::Json;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize, PartialEq, Serialize, ToSchema)]
pub struct AiCitationAttachment {
    pub id: Uuid,
    pub file_name: String,
    pub mime_type: String,
    pub size_bytes: i64,
    pub download_url: String,
    pub is_sensitive: bool,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Serialize, ToSchema)]
pub struct AiCitationSource {
    pub label: String,
    pub room_id: Uuid,
    pub message_id: Uuid,
    pub sender: String,
    pub sent_at: DateTime<Utc>,
    pub excerpt: String,
    #[serde(default)]
    pub score: Option<f64>,
    #[serde(default = "vector_score_kind")]
    pub score_kind: String,
    #[serde(default)]
    pub attachment: Option<AiCitationAttachment>,
}

fn vector_score_kind() -> String {
    "vector".into()
}

#[derive(Debug, Clone, Deserialize, PartialEq, Serialize, ToSchema)]
pub struct AiRunTraceStep {
    pub key: String,
    pub label: String,
    pub detail: String,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

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
    pub retrieved_message_count: Option<i64>,
    #[schema(value_type = Vec<AiCitationSource>)]
    pub sources: Json<Vec<AiCitationSource>>,
    #[schema(value_type = Vec<AiRunTraceStep>)]
    pub trace: Json<Vec<AiRunTraceStep>>,
    pub status: String,
    pub stage: String,
    pub stage_started_at: Option<DateTime<Utc>>,
    pub revision: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct AiRun {
    pub id: Uuid,
    pub thread_id: Uuid,
    pub user_message_id: Uuid,
    pub assistant_message_id: Uuid,
    pub client_request_id: Uuid,
    pub room_id: Option<Uuid>,
    pub purpose: String,
    pub source_after_message_id: Option<Uuid>,
    pub source_through_message_id: Option<Uuid>,
    pub source_message_count: Option<i64>,
    pub model_option_id: Option<Uuid>,
    pub provider: String,
    pub model: String,
    pub status: String,
    pub context_message_count: Option<i64>,
    pub retrieved_message_count: Option<i64>,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
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
pub struct CreateAiRunRequest {
    pub question: String,
    pub room_id: Option<Uuid>,
    pub model_option_id: Option<Uuid>,
    pub client_request_id: Uuid,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateCatchUpRunRequest {
    pub room_id: Uuid,
    pub model_option_id: Option<Uuid>,
    pub client_request_id: Uuid,
}
