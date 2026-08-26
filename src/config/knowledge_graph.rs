use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct KnowledgeGraphConfig {
    pub enabled: bool,
    pub url: String,
    pub api_token_env: String,
    pub max_facts: usize,
    pub graph_limit: usize,
    pub worker_interval_ms: u64,
    pub request_timeout_secs: u64,
    pub search_timeout_ms: u64,
    pub worker_concurrency: usize,
}

impl KnowledgeGraphConfig {
    pub(crate) fn api_token(&self) -> Option<String> {
        let name = self.api_token_env.trim();
        if name.is_empty() {
            return None;
        }
        std::env::var(name)
            .ok()
            .filter(|value| !value.trim().is_empty())
    }
}

impl Default for KnowledgeGraphConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            url: "http://127.0.0.1:8090".into(),
            api_token_env: "CHAT_ROOM_KNOWLEDGE_GRAPH_TOKEN".into(),
            max_facts: 8,
            graph_limit: 250,
            worker_interval_ms: 1_000,
            request_timeout_secs: 180,
            search_timeout_ms: 2_000,
            worker_concurrency: 4,
        }
    }
}
