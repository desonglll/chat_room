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
