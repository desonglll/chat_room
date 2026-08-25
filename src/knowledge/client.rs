use anyhow::{bail, Context, Result};
use reqwest::{Client, RequestBuilder, StatusCode};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::config::VectorStoreConfig;

#[derive(Clone)]
pub(super) struct VectorClients {
    http: Client,
    config: VectorStoreConfig,
    qdrant_api_key: Option<String>,
    embedding_api_key: Option<String>,
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

    pub(super) async fn search(&self, room_id: Uuid, vector: Vec<f32>) -> Result<Vec<Uuid>> {
        let url = format!("{}/points/search", self.collection_url());
        let response: SearchResponse = self
            .qdrant(self.http.post(url))
            .json(&json!({
                "vector": vector,
                "filter": { "must": [{ "key": "room_id", "match": { "value": room_id } }] },
                "limit": self.config.top_k,
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
        Ok(response
            .result
            .into_iter()
            .filter_map(|point| {
                point
                    .payload
                    .get("message_id")
                    .and_then(Value::as_str)
                    .and_then(|id| Uuid::parse_str(id).ok())
            })
            .collect())
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
    payload: serde_json::Map<String, Value>,
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
