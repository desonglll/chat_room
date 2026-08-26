mod client;
mod rag;
mod store;
mod worker;

pub(crate) use rag::retrieve_room_context;
pub use worker::ensure_worker;

use anyhow::Result;
use serde::Serialize;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use uuid::Uuid;

use self::client::{ScoredMessageId, VectorClients};
use crate::config::VectorStoreConfig;

#[derive(Clone)]
pub struct MessageIndex {
    clients: VectorClients,
    worker_interval: std::time::Duration,
    collection_ready: Arc<AtomicBool>,
}

impl MessageIndex {
    pub async fn connect(config: &VectorStoreConfig) -> Result<Option<Self>> {
        if !config.enabled {
            return Ok(None);
        }
        let clients = VectorClients::new(config)?;
        Ok(Some(Self {
            clients,
            worker_interval: std::time::Duration::from_millis(config.worker_interval_ms),
            collection_ready: Arc::new(AtomicBool::new(false)),
        }))
    }

    async fn ensure_ready(&self) -> Result<()> {
        if self.collection_ready.load(Ordering::Acquire) {
            return Ok(());
        }
        self.clients.ensure_collection().await?;
        self.collection_ready.store(true, Ordering::Release);
        Ok(())
    }

    pub(crate) async fn related_messages(
        &self,
        room_id: Uuid,
        question: &str,
        excluded_message_ids: &std::collections::HashSet<Uuid>,
    ) -> Result<Vec<ScoredMessageId>> {
        let vector = self.embed_question(question).await?;
        self.search_vector(room_id, vector, excluded_message_ids)
            .await
    }

    pub(crate) async fn embed_question(&self, question: &str) -> Result<Vec<f32>> {
        self.clients.embed(question).await
    }

    pub(crate) async fn search_vector(
        &self,
        room_id: Uuid,
        vector: Vec<f32>,
        excluded_message_ids: &std::collections::HashSet<Uuid>,
    ) -> Result<Vec<ScoredMessageId>> {
        self.ensure_ready().await?;
        self.clients
            .search(room_id, vector, excluded_message_ids)
            .await
    }

    pub(crate) async fn rerank(
        &self,
        query: &str,
        documents: &[String],
    ) -> Result<Vec<client::RerankScore>> {
        self.clients.rerank(query, documents).await
    }

    pub(crate) fn embedding_model(&self) -> &str {
        self.clients.embedding_model()
    }

    pub(crate) fn rerank_model(&self) -> Option<&str> {
        self.clients.rerank_model()
    }

    pub(crate) fn result_limit(&self) -> usize {
        self.clients.result_limit()
    }

    pub(crate) async fn point_count(&self) -> Result<u64> {
        self.ensure_ready().await?;
        self.clients.point_count().await
    }
}

#[derive(Clone, Debug, Serialize, sqlx::FromRow)]
pub(crate) struct RetrievedMessage {
    pub id: Uuid,
    pub sender: String,
    pub content: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub attachment_id: Option<Uuid>,
    pub attachment_access_key: Option<Uuid>,
    pub attachment_file_name: Option<String>,
    pub attachment_mime_type: Option<String>,
    pub attachment_size_bytes: Option<i64>,
    pub attachment_is_sensitive: Option<bool>,
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::sync::{Arc, Mutex};

    use axum::{
        extract::State,
        http::{Method, StatusCode, Uri},
        response::IntoResponse,
        routing::any,
        Json, Router,
    };

    use super::*;

    type FakeServiceCalls = Arc<Mutex<Vec<(Method, String, serde_json::Value)>>>;

    #[tokio::test]
    async fn openai_compatible_embeddings_drive_room_filtered_qdrant_search() {
        async fn fake_services(
            State(calls): State<FakeServiceCalls>,
            method: Method,
            uri: Uri,
            payload: Option<Json<serde_json::Value>>,
        ) -> impl IntoResponse {
            let body = payload.map_or(serde_json::Value::Null, |Json(value)| value);
            calls
                .lock()
                .unwrap()
                .push((method.clone(), uri.to_string(), body));
            if method == Method::GET && uri.path() == "/collections/messages" {
                return (StatusCode::NOT_FOUND, Json(serde_json::json!({}))).into_response();
            }
            if uri.path() == "/embeddings" {
                return Json(serde_json::json!({
                    "data": [{ "index": 0, "embedding": [0.1, 0.2] }]
                }))
                .into_response();
            }
            if uri.path() == "/rerank" {
                return Json(serde_json::json!({
                    "results": [
                        { "index": 1, "relevance_score": 0.91 },
                        { "index": 0, "relevance_score": 0.32 }
                    ]
                }))
                .into_response();
            }
            if uri.path().ends_with("/points/search") {
                return Json(serde_json::json!({
                    "result": [{
                        "score": 0.82,
                        "payload": { "message_id": "d9ceaaae-c068-4a22-81cb-d6c29ee46b9a" }
                    }]
                }))
                .into_response();
            }
            Json(serde_json::json!({ "result": {} })).into_response()
        }

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let calls = Arc::new(Mutex::new(Vec::new()));
        let server_calls = calls.clone();
        let server = tokio::spawn(async move {
            axum::serve(
                listener,
                Router::new()
                    .fallback(any(fake_services))
                    .with_state(server_calls),
            )
            .await
            .unwrap();
        });
        let base_url = format!("http://{address}");
        let config = VectorStoreConfig {
            enabled: true,
            url: base_url.clone(),
            collection: "messages".into(),
            dimensions: 2,
            embedding_base_url: base_url.clone(),
            embedding_model: "embed-test".into(),
            rerank_enabled: true,
            rerank_base_url: base_url,
            rerank_model: "rerank-test".into(),
            rerank_score_threshold: 0.5,
            ..VectorStoreConfig::default()
        };

        let index = MessageIndex::connect(&config).await.unwrap().unwrap();
        let room_id = Uuid::new_v4();
        let recent_id = Uuid::new_v4();
        let matches = index
            .related_messages(room_id, "release plan", &HashSet::from([recent_id]))
            .await
            .unwrap();
        let reranked = index
            .rerank("release plan", &["first".into(), "second".into()])
            .await
            .unwrap();
        index
            .clients
            .upsert(Uuid::new_v4(), room_id, vec![0.1, 0.2])
            .await
            .unwrap();

        assert_eq!(
            matches,
            [ScoredMessageId {
                id: Uuid::parse_str("d9ceaaae-c068-4a22-81cb-d6c29ee46b9a").unwrap(),
                score: 0.82,
            }]
        );
        assert_eq!(
            reranked,
            [client::RerankScore {
                index: 1,
                score: 0.91,
            }]
        );
        let calls = calls.lock().unwrap();
        assert!(calls.iter().any(|(method, uri, _)| {
            method == Method::PUT && uri.starts_with("/collections/messages/index")
        }));
        let search = calls
            .iter()
            .find(|(_, uri, _)| uri == "/collections/messages/points/search")
            .unwrap();
        assert_eq!(
            search.2["filter"]["must"][0]["match"]["value"],
            room_id.to_string()
        );
        assert_eq!(
            search.2["filter"]["must_not"][0]["has_id"][0],
            recent_id.to_string()
        );
        assert_eq!(search.2["limit"], 18);
        assert!(calls.iter().any(|(method, uri, body)| {
            method == Method::PUT
                && uri.starts_with("/collections/messages/points?wait=true")
                && body["points"][0]["payload"]["room_id"] == room_id.to_string()
        }));
        let rerank = calls.iter().find(|(_, uri, _)| uri == "/rerank").unwrap();
        assert_eq!(rerank.2["model"], "rerank-test");
        assert_eq!(rerank.2["top_n"], 2);
        server.abort();
    }
}
