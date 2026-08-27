use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct RoomAiPolicy {
    pub room_id: Uuid,
    pub mode: String,
    pub version: i64,
    pub applies_to: String,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateRoomAiPolicy {
    pub mode: String,
    pub version: i64,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct AiGovernedModel {
    pub id: Uuid,
    pub label: String,
    pub provider: String,
    pub model: String,
    pub ready: bool,
    pub allowed: bool,
    pub input_price_micros_per_million: i64,
    pub output_price_micros_per_million: i64,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct AiGovernanceSettings {
    pub max_concurrent_runs: i64,
    pub daily_user_token_limit: Option<i64>,
    pub daily_room_token_limit: Option<i64>,
    pub allowlist_enabled: bool,
    pub models: Vec<AiGovernedModel>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateAiGovernanceModel {
    pub id: Uuid,
    pub allowed: bool,
    pub input_price_micros_per_million: i64,
    pub output_price_micros_per_million: i64,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateAiGovernanceSettings {
    pub max_concurrent_runs: i64,
    pub daily_user_token_limit: Option<i64>,
    pub daily_room_token_limit: Option<i64>,
    pub allowlist_enabled: bool,
    pub models: Vec<UpdateAiGovernanceModel>,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct AiUsageQuery {
    pub group_by: Option<String>,
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, Serialize, ToSchema, sqlx::FromRow)]
pub struct AiUsageAggregate {
    pub key: String,
    pub label: String,
    pub runs: i64,
    pub completed_runs: i64,
    pub failed_runs: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub total_tokens: i64,
    pub duration_ms: i64,
    pub estimated_cost_micros: i64,
}

#[derive(Debug, sqlx::FromRow)]
pub(super) struct AiUsageAggregateRow {
    pub group_id: Option<Uuid>,
    pub label: Option<String>,
    pub runs: i64,
    pub completed_runs: i64,
    pub failed_runs: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub total_tokens: i64,
    pub duration_ms: i64,
    pub estimated_cost_micros: i64,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct AiUsageReport {
    pub group_by: String,
    pub from: DateTime<Utc>,
    pub to: DateTime<Utc>,
    pub token_source: String,
    pub items: Vec<AiUsageAggregate>,
}

#[derive(Debug, sqlx::FromRow)]
pub(super) struct GovernanceSettingsRow {
    pub max_concurrent_runs: i64,
    pub daily_user_token_limit: Option<i64>,
    pub daily_room_token_limit: Option<i64>,
    pub allowlist_enabled: bool,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, sqlx::FromRow)]
pub(super) struct GovernanceModelRow {
    pub model_option_id: Uuid,
    pub allowed: bool,
    pub input_price_micros_per_million: i64,
    pub output_price_micros_per_million: i64,
}
