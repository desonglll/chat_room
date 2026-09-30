//! Qdrant collection, embedding profile, and optional reranking profile.

use anyhow::{bail, Result};
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct VectorStoreConfig {
    pub enabled: bool,
    pub url: String,
    pub collection: String,
    pub api_key_env: String,
    pub dimensions: usize,
    pub top_k: usize,
    pub score_threshold: f32,
    pub embedding_base_url: String,
    pub embedding_model: String,
    pub embedding_api_key_env: String,
    pub rerank_enabled: bool,
    pub rerank_base_url: String,
    pub rerank_model: String,
    pub rerank_api_key_env: String,
    pub rerank_timeout_ms: u64,
    pub rerank_score_threshold: f32,
    pub worker_interval_ms: u64,
}

impl VectorStoreConfig {
    pub(crate) fn qdrant_api_key(&self) -> Option<String> {
        env_value(&self.api_key_env)
    }

    pub(crate) fn embedding_api_key(&self) -> Option<String> {
        env_value(&self.embedding_api_key_env)
    }

    pub(crate) fn rerank_api_key(&self) -> Option<String> {
        env_value(&self.rerank_api_key_env)
    }

    pub(super) fn validate(&self) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }
        if self.url.trim().is_empty()
            || self.collection.trim().is_empty()
            || self.embedding_base_url.trim().is_empty()
            || self.embedding_model.trim().is_empty()
        {
            bail!("vector_store requires url, collection, embedding_base_url, and embedding_model when enabled");
        }
        if self.dimensions == 0 || self.dimensions > 65_536 {
            bail!("vector_store.dimensions must be between 1 and 65536");
        }
        if self.top_k == 0 || self.top_k > 50 {
            bail!("vector_store.top_k must be between 1 and 50");
        }
        if !(0.0..=1.0).contains(&self.score_threshold) {
            bail!("vector_store.score_threshold must be between 0 and 1");
        }
        self.validate_reranking()?;
        if self.worker_interval_ms == 0 {
            bail!("vector_store.worker_interval_ms must be greater than zero");
        }
        // The collection name reaches Qdrant's URL path, so restrict it to
        // characters that cannot change the request's shape.
        if !self
            .collection
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-'))
        {
            bail!("vector_store.collection may contain only ASCII letters, numbers, '_' and '-'");
        }
        Ok(())
    }

    fn validate_reranking(&self) -> Result<()> {
        if !self.rerank_enabled {
            return Ok(());
        }
        if self.rerank_base_url.trim().is_empty() || self.rerank_model.trim().is_empty() {
            bail!("vector_store reranking requires rerank_base_url and rerank_model");
        }
        if self.rerank_timeout_ms == 0 || self.rerank_timeout_ms > 30_000 {
            bail!("vector_store.rerank_timeout_ms must be between 1 and 30000");
        }
        if !(0.0..=1.0).contains(&self.rerank_score_threshold) {
            bail!("vector_store.rerank_score_threshold must be between 0 and 1");
        }
        Ok(())
    }
}

impl Default for VectorStoreConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            url: "http://127.0.0.1:6333".into(),
            collection: "chat_messages".into(),
            api_key_env: String::new(),
            dimensions: 1024,
            top_k: 6,
            score_threshold: 0.55,
            embedding_base_url: String::new(),
            embedding_model: String::new(),
            embedding_api_key_env: String::new(),
            rerank_enabled: false,
            rerank_base_url: String::new(),
            rerank_model: String::new(),
            rerank_api_key_env: String::new(),
            rerank_timeout_ms: 2_000,
            rerank_score_threshold: 0.35,
            worker_interval_ms: 500,
        }
    }
}

fn env_value(name: &str) -> Option<String> {
    let name = name.trim();
    if name.is_empty() {
        return None;
    }
    std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
}
