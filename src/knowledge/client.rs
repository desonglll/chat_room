use std::collections::HashSet;

use anyhow::{bail, Context, Result};
use reqwest::{Client, RequestBuilder, StatusCode};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::config::VectorStoreConfig;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ScoredMessageId {
    pub id: Uuid,
    pub score: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RerankScore {
    pub index: usize,
    pub score: f64,
}

#[derive(Clone)]
pub(super) struct VectorClients {
    http: Client,
    config: VectorStoreConfig,
    qdrant_api_key: Option<String>,
    embedding_api_key: Option<String>,
    rerank_api_key: Option<String>,
}

impl VectorClients {
    pub(super) fn new(config: &VectorStoreConfig) -> Result<Self> {
        Ok(Self {
            http: Client::builder()
                .connect_timeout(std::time::Duration::from_secs(3))
                .timeout(std::time::Duration::from_secs(30))
                .build()?,
            config: config.clone(),
            qdrant_api_key: config.qdrant_api_key(),
            embedding_api_key: config.embedding_api_key(),
            rerank_api_key: config.rerank_api_key(),
        })
    }

    pub(super) async fn ensure_collection(&self) -> Result<()> {
        let collection_url = self.collection_url();
        let response = self.qdrant(self.http.get(&collection_url)).send().await?;
        if response.status() == StatusCode::NOT_FOUND {
            self.qdrant(self.http.put(&collection_url))
                .json(&json!({
                    "vectors": { "size": self.config.dimensions, "distance": "Cosine" },
                    "on_disk_payload": true
                }))
                .send()
                .await?
                .error_for_status()
                .context("create Qdrant collection")?;
        } else {
            response
                .error_for_status()
                .context("inspect Qdrant collection")?;
        }
        self.ensure_keyword_index("room_id").await?;
        self.ensure_keyword_index("message_id").await
    }

    pub(super) async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let url = format!(
            "{}/embeddings",
            self.config.embedding_base_url.trim_end_matches('/')
        );
        let mut request = self.http.post(url).json(&json!({
            "model": self.config.embedding_model,
            "input": [text],
            "encoding_format": "float"
        }));
        if let Some(api_key) = &self.embedding_api_key {
            request = request.bearer_auth(api_key);
        }
        let response: EmbeddingResponse = request
            .send()
            .await?
            .error_for_status()
            .context("embedding request failed")?
            .json()
            .await
            .context("decode embedding response")?;
        let embedding = response
            .data
            .into_iter()
            .min_by_key(|item| item.index)
            .map(|item| item.embedding)
            .ok_or_else(|| anyhow::anyhow!("embedding response contained no vectors"))?;
        if embedding.len() != self.config.dimensions {
            bail!(
                "embedding dimensions mismatch: expected {}, received {}",
                self.config.dimensions,
                embedding.len()
            );
        }
        Ok(embedding)
    }

    pub(super) async fn upsert(
        &self,
        message_id: Uuid,
        room_id: Uuid,
        vector: Vec<f32>,
    ) -> Result<()> {
        let url = format!("{}/points?wait=true", self.collection_url());
        self.qdrant(self.http.put(url))
            .json(&json!({
                "points": [{
                    "id": message_id,
                    "vector": vector,
                    "payload": { "message_id": message_id, "room_id": room_id }
                }]
            }))
            .send()
            .await?
            .error_for_status()
            .context("upsert Qdrant point")?;
        Ok(())
    }

    pub(super) async fn delete(&self, message_id: Uuid) -> Result<()> {
        let url = format!("{}/points/delete?wait=true", self.collection_url());
        self.qdrant(self.http.post(url))
            .json(&json!({ "points": [message_id] }))
            .send()
            .await?
            .error_for_status()
            .context("delete Qdrant point")?;
        Ok(())
    }

    pub(super) async fn search(
        &self,
        room_id: Uuid,
        vector: Vec<f32>,
        excluded_message_ids: &HashSet<Uuid>,
    ) -> Result<Vec<ScoredMessageId>> {
        let url = format!("{}/points/search", self.collection_url());
        let mut filter = json!({
            "must": [{ "key": "room_id", "match": { "value": room_id } }]
        });
        if !excluded_message_ids.is_empty() {
            let excluded: Vec<String> = excluded_message_ids.iter().map(Uuid::to_string).collect();
            filter["must_not"] = json!([{ "has_id": excluded }]);
        }
        let response: SearchResponse = self
            .qdrant(self.http.post(url))
            .json(&json!({
                "vector": vector,
                "filter": filter,
                "limit": self.candidate_limit(),
                "score_threshold": self.config.score_threshold,
                "with_payload": true,
                "with_vector": false
            }))
            .send()
            .await?
            .error_for_status()
            .context("search Qdrant points")?
            .json()
            .await
            .context("decode Qdrant search response")?;
        let candidates = response
            .result
            .into_iter()
            .filter_map(|point| {
                let id = point
                    .payload
                    .get("message_id")
                    .and_then(Value::as_str)
                    .and_then(|id| Uuid::parse_str(id).ok())?;
                Some(ScoredMessageId {
                    id,
                    score: point.score,
                })
            })
            .collect();
        Ok(select_vector_candidates(
            candidates,
            self.candidate_limit(),
            f64::from(self.config.score_threshold),
        ))
    }

    pub(super) async fn rerank(
        &self,
        query: &str,
        documents: &[String],
    ) -> Result<Vec<RerankScore>> {
        if documents.is_empty() {
            return Ok(Vec::new());
        }
        let url = format!(
            "{}/rerank",
            self.config.rerank_base_url.trim_end_matches('/')
        );
        let mut request = self.http.post(url).json(&json!({
            "model": self.config.rerank_model,
            "query": query,
            "documents": documents,
            "return_documents": false,
            "top_n": self.config.top_k.min(documents.len())
        }));
        if let Some(api_key) = &self.rerank_api_key {
            request = request.bearer_auth(api_key);
        }
        let response: RerankResponse = tokio::time::timeout(
            std::time::Duration::from_millis(self.config.rerank_timeout_ms),
            async {
                request
                    .send()
                    .await?
                    .error_for_status()
                    .context("rerank request failed")?
                    .json()
                    .await
                    .context("decode rerank response")
            },
        )
        .await
        .context("rerank request timed out")??;
        Ok(response
            .results
            .into_iter()
            .filter(|result| {
                result.index < documents.len()
                    && result.relevance_score.is_finite()
                    && result.relevance_score >= f64::from(self.config.rerank_score_threshold)
            })
            .map(|result| RerankScore {
                index: result.index,
                score: result.relevance_score,
            })
            .collect())
    }

    pub(super) fn result_limit(&self) -> usize {
        self.config.top_k
    }

    pub(super) fn embedding_model(&self) -> &str {
        &self.config.embedding_model
    }

    pub(super) fn rerank_model(&self) -> Option<&str> {
        self.config
            .rerank_enabled
            .then_some(self.config.rerank_model.as_str())
    }

    fn candidate_limit(&self) -> usize {
        self.config.top_k.saturating_mul(3).min(50)
    }

    pub(super) async fn point_count(&self) -> Result<u64> {
        let response: CollectionResponse = self
            .qdrant(self.http.get(self.collection_url()))
            .send()
            .await?
            .error_for_status()
            .context("inspect Qdrant collection health")?
            .json()
            .await
            .context("decode Qdrant collection health")?;
        Ok(response.result.points_count)
    }

    async fn ensure_keyword_index(&self, field_name: &str) -> Result<()> {
        let url = format!("{}/index?wait=true", self.collection_url());
        self.qdrant(self.http.put(url))
            .json(&json!({ "field_name": field_name, "field_schema": "keyword" }))
            .send()
            .await?
            .error_for_status()
            .with_context(|| format!("create Qdrant payload index {field_name}"))?;
        Ok(())
    }

    fn collection_url(&self) -> String {
        format!(
            "{}/collections/{}",
            self.config.url.trim_end_matches('/'),
            self.config.collection
        )
    }

    fn qdrant(&self, request: RequestBuilder) -> RequestBuilder {
        match &self.qdrant_api_key {
            Some(api_key) => request.header("api-key", api_key),
            None => request,
        }
    }
}

#[derive(Deserialize)]
struct EmbeddingResponse {
    data: Vec<EmbeddingData>,
}

#[derive(Deserialize)]
struct EmbeddingData {
    index: usize,
    embedding: Vec<f32>,
}

#[derive(Deserialize)]
struct SearchResponse {
    result: Vec<SearchPoint>,
}

#[derive(Deserialize)]
struct SearchPoint {
    #[serde(default)]
    score: f64,
    #[serde(default)]
    payload: serde_json::Map<String, Value>,
}

#[derive(Deserialize)]
struct RerankResponse {
    results: Vec<RerankResult>,
}

#[derive(Deserialize)]
struct RerankResult {
    index: usize,
    relevance_score: f64,
}

#[derive(Deserialize)]
struct CollectionResponse {
    result: CollectionInfo,
}

#[derive(Deserialize)]
struct CollectionInfo {
    #[serde(default)]
    points_count: u64,
}

fn select_vector_candidates(
    mut candidates: Vec<ScoredMessageId>,
    limit: usize,
    minimum_score: f64,
) -> Vec<ScoredMessageId> {
    candidates.retain(|candidate| candidate.score.is_finite());
    candidates.sort_by(|left, right| right.score.total_cmp(&left.score));
    candidates.retain(|candidate| candidate.score >= minimum_score);
    candidates.truncate(limit);
    candidates
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semantic_candidates_must_clear_absolute_and_relative_thresholds() {
        let candidates = [0.58, 0.77, 0.70, 0.62]
            .into_iter()
            .map(|score| ScoredMessageId {
                id: Uuid::new_v4(),
                score,
            })
            .collect();

        let selected = select_vector_candidates(candidates, 6, 0.55);

        assert_eq!(
            selected.iter().map(|item| item.score).collect::<Vec<_>>(),
            [0.77, 0.70, 0.62, 0.58]
        );
    }
}
