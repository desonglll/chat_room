mod client;
mod rag;
mod store;
mod worker;

pub(crate) use rag::retrieve_room_context;
pub use worker::ensure_worker;

use anyhow::Result;
use serde::Serialize;
use uuid::Uuid;

use self::client::{ScoredMessageId, VectorClients};
use crate::config::VectorStoreConfig;

#[derive(Clone)]
pub struct MessageIndex {
    clients: VectorClients,
    worker_interval: std::time::Duration,
}

impl MessageIndex {
    pub async fn connect(config: &VectorStoreConfig) -> Result<Option<Self>> {
        if !config.enabled {
            return Ok(None);
        }
        let clients = VectorClients::new(config)?;
        clients.ensure_collection().await?;
        Ok(Some(Self {
            clients,
            worker_interval: std::time::Duration::from_millis(config.worker_interval_ms),
        }))
    }

    pub(crate) async fn related_messages(
        &self,
        room_id: Uuid,
        question: &str,
        excluded_message_ids: &std::collections::HashSet<Uuid>,
    ) -> Result<Vec<ScoredMessageId>> {
        let vector = self.clients.embed(question).await?;
        self.clients
            .search(room_id, vector, excluded_message_ids)
            .await
    }

    pub(crate) fn result_limit(&self) -> usize {
        self.clients.result_limit()
    }

    pub(crate) async fn point_count(&self) -> Result<u64> {
        self.clients.point_count().await
    }
}

#[derive(Clone, Debug, Serialize, sqlx::FromRow)]
pub(crate) struct RetrievedMessage {
    pub id: Uuid,
    pub sender: String,
    pub content: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
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

    #[tokio::test]
    async fn openai_compatible_embeddings_drive_room_filtered_qdrant_search() {
        async fn fake_services(
            State(calls): State<Arc<Mutex<Vec<(Method, String, serde_json::Value)>>>>,
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
            embedding_base_url: base_url,
            embedding_model: "embed-test".into(),
            ..VectorStoreConfig::default()
        };

        let index = MessageIndex::connect(&config).await.unwrap().unwrap();
        let room_id = Uuid::new_v4();
        let recent_id = Uuid::new_v4();
        let matches = index
            .related_messages(room_id, "release plan", &HashSet::from([recent_id]))
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
        server.abort();
    }
}
