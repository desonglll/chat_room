use std::time::Duration;

use anyhow::{Context, Result};
use reqwest::Client;
use uuid::Uuid;

use super::models::{EpisodeUpsert, GraphFact, GraphSnapshot, SearchRequest, SearchResponse};
use crate::config::KnowledgeGraphConfig;

#[derive(Clone)]
pub struct KnowledgeGraph {
    http: Client,
    base_url: String,
    token: String,
    pub(super) worker_interval: Duration,
    pub(super) worker_concurrency: usize,
    max_facts: usize,
    graph_limit: usize,
    pub(crate) search_timeout: Duration,
}

impl KnowledgeGraph {
    pub fn connect(config: &KnowledgeGraphConfig) -> Result<Option<Self>> {
        if !config.enabled {
            return Ok(None);
        }
        let token = config.api_token().ok_or_else(|| {
            anyhow::anyhow!("{} is missing or empty", config.api_token_env.trim())
        })?;
        Ok(Some(Self {
            http: Client::builder()
                .connect_timeout(Duration::from_secs(3))
                .timeout(Duration::from_secs(config.request_timeout_secs))
                .build()?,
            base_url: config.url.trim_end_matches('/').to_owned(),
            token,
            worker_interval: Duration::from_millis(config.worker_interval_ms),
            worker_concurrency: config.worker_concurrency,
            max_facts: config.max_facts,
            graph_limit: config.graph_limit,
            search_timeout: Duration::from_millis(config.search_timeout_ms),
        }))
    }

    pub(super) async fn upsert(&self, message_id: Uuid, episode: &EpisodeUpsert<'_>) -> Result<()> {
        self.http
            .put(format!("{}/v1/episodes/{message_id}", self.base_url))
            .bearer_auth(&self.token)
            .json(episode)
            .send()
            .await?
            .error_for_status()
            .context("upsert graph episode")?;
        Ok(())
    }

    pub(super) async fn delete(&self, room_id: Uuid, message_id: Uuid) -> Result<()> {
        self.http
            .delete(format!("{}/v1/episodes/{message_id}", self.base_url))
            .bearer_auth(&self.token)
            .query(&[("room_id", room_id)])
            .send()
            .await?
            .error_for_status()
            .context("delete graph episode")?;
        Ok(())
    }

    pub(crate) async fn search(&self, room_id: Uuid, query: &str) -> Result<Vec<GraphFact>> {
        let response: SearchResponse = self
            .http
            .post(format!("{}/v1/search", self.base_url))
            .bearer_auth(&self.token)
            .json(&SearchRequest {
                room_id,
                query,
                limit: self.max_facts,
            })
            .send()
            .await?
            .error_for_status()
            .context("search room graph")?
            .json()
            .await
            .context("decode graph search response")?;
        Ok(response.facts)
    }

    pub(crate) async fn snapshot(&self, room_id: Uuid) -> Result<GraphSnapshot> {
        let snapshot: GraphSnapshot = self
            .http
            .get(format!("{}/v1/rooms/{room_id}/graph", self.base_url))
            .bearer_auth(&self.token)
            .query(&[("limit", self.graph_limit)])
            .send()
            .await?
            .error_for_status()
            .context("load room graph")?
            .json()
            .await
            .context("decode room graph")?;
        anyhow::ensure!(snapshot.room_id == room_id, "graph response room mismatch");
        Ok(snapshot)
    }

    pub(crate) async fn health(&self) -> Result<()> {
        self.http
            .get(format!("{}/healthz", self.base_url))
            .send()
            .await?
            .error_for_status()
            .context("graph service health")?;
        Ok(())
    }
}
