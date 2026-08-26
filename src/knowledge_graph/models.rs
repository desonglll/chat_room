use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct GraphNode {
    pub id: Uuid,
    pub name: String,
    pub summary: String,
    pub labels: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct GraphFact {
    pub id: Uuid,
    pub name: String,
    pub fact: String,
    pub source_node_id: Uuid,
    pub target_node_id: Uuid,
    pub episode_ids: Vec<Uuid>,
    pub valid_at: Option<DateTime<Utc>>,
    pub invalid_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub expired_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct GraphSnapshot {
    pub room_id: Uuid,
    pub nodes: Vec<GraphNode>,
    pub facts: Vec<GraphFact>,
    pub truncated: bool,
}

#[derive(Deserialize)]
pub(super) struct SearchResponse {
    pub facts: Vec<GraphFact>,
}

#[derive(Serialize)]
pub(super) struct SearchRequest<'a> {
    pub room_id: Uuid,
    pub query: &'a str,
    pub limit: usize,
}

#[derive(Serialize)]
pub(super) struct EpisodeUpsert<'a> {
    pub room_id: Uuid,
    pub sender: &'a str,
    pub content: &'a str,
    pub created_at: DateTime<Utc>,
}
