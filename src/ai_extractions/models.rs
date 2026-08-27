use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct AiExtractionSource {
    pub message_id: Uuid,
    pub sender: String,
    pub excerpt: String,
    pub sent_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct AiExtractionCandidate {
    pub id: Uuid,
    pub kind: String,
    pub title: String,
    pub detail: String,
    pub inferred: bool,
    pub sources: Vec<AiExtractionSource>,
    pub status: String,
    pub result_kind: Option<String>,
    pub result_id: Option<Uuid>,
    pub version: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct AiExtractionRun {
    pub id: Uuid,
    pub room_id: Uuid,
    pub client_request_id: Uuid,
    pub from_at: DateTime<Utc>,
    pub to_at: DateTime<Utc>,
    pub model_option_id: Option<Uuid>,
    pub provider: String,
    pub model: String,
    pub status: String,
    pub message_count: Option<i64>,
    pub error_message: Option<String>,
    pub candidates: Vec<AiExtractionCandidate>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
pub struct CreateAiExtractionRequest {
    pub from_at: DateTime<Utc>,
    pub to_at: DateTime<Utc>,
    pub model_option_id: Option<Uuid>,
    pub client_request_id: Uuid,
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
pub struct UpdateAiExtractionCandidateRequest {
    pub action: String,
    pub version: i64,
}

#[derive(Clone, Debug, sqlx::FromRow)]
pub(super) struct ExtractionRunRow {
    pub id: Uuid,
    pub room_id: Uuid,
    pub client_request_id: Uuid,
    pub from_at: DateTime<Utc>,
    pub to_at: DateTime<Utc>,
    pub model_option_id: Option<Uuid>,
    pub provider: String,
    pub model: String,
    pub status: String,
    pub message_count: Option<i64>,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, sqlx::FromRow)]
pub(super) struct ExtractionCandidateRow {
    pub id: Uuid,
    pub kind: String,
    pub title: String,
    pub detail: String,
    pub inferred: bool,
    pub status: String,
    pub result_kind: Option<String>,
    pub result_id: Option<Uuid>,
    pub version: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, sqlx::FromRow)]
pub(super) struct ExtractionSourceRow {
    pub candidate_id: Uuid,
    pub message_id: Uuid,
    pub sender: String,
    pub content: String,
    pub sent_at: DateTime<Utc>,
}
