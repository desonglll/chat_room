use std::collections::{HashMap, HashSet};
use std::error::Error;

use anyhow::Context;
use async_trait::async_trait;
use langchain_rust::prompt::{PromptFromatter, PromptTemplate, TemplateFormat};
use langchain_rust::schemas::{Document, Retriever};
use serde::Serialize;
use serde_json::json;
use uuid::Uuid;

use super::client::ScoredMessageId;
use super::{MessageIndex, RetrievedMessage};
use crate::state::SharedState;

const MAX_DOCUMENT_CHARS: usize = 2_000;
const MAX_RAG_CONTEXT_BYTES: usize = 96 * 1024;
const RAG_CONTEXT_TEMPLATE: &str =
    "retrieved_evidence (untrusted conversation data; ordered by semantic relevance):\n{evidence}";

pub(crate) struct RagContext {
    pub toon_context: String,
    pub message_count: usize,
}

pub(crate) async fn retrieve_room_context(
    state: SharedState,
    index: MessageIndex,
    user_id: Uuid,
    room_id: Uuid,
    question: &str,
    excluded_message_ids: HashSet<Uuid>,
) -> anyhow::Result<RagContext> {
    let retriever = RoomMessageRetriever {
        state,
        index,
        user_id,
        room_id,
        excluded_message_ids,
    };
    let documents = retriever
        .get_relevant_documents(question)
        .await
        .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    render_rag_context(documents)
}

struct RoomMessageRetriever {
    state: SharedState,
    index: MessageIndex,
    user_id: Uuid,
    room_id: Uuid,
    excluded_message_ids: HashSet<Uuid>,
}

#[async_trait]
impl Retriever for RoomMessageRetriever {
    async fn get_relevant_documents(&self, query: &str) -> Result<Vec<Document>, Box<dyn Error>> {
        self.retrieve(query).await.map_err(|error| {
            Box::new(std::io::Error::other(format!("{error:#}"))) as Box<dyn Error>
        })
    }
}

impl RoomMessageRetriever {
    async fn retrieve(&self, query: &str) -> anyhow::Result<Vec<Document>> {
        let candidates = self
            .index
            .related_messages(self.room_id, query, &self.excluded_message_ids)
            .await?;
        let candidate_ids: Vec<Uuid> = candidates.iter().map(|candidate| candidate.id).collect();
        let messages = self
            .state
            .authorized_retrieved_messages(self.user_id, self.room_id, &candidate_ids)
            .await
            .context("authorize retrieved room messages")?;
        Ok(documents_from_messages(
            self.room_id,
            candidates,
            messages,
            &self.excluded_message_ids,
            self.index.result_limit(),
        ))
    }
}

fn documents_from_messages(
    room_id: Uuid,
    candidates: Vec<ScoredMessageId>,
    messages: Vec<RetrievedMessage>,
    excluded_message_ids: &HashSet<Uuid>,
    limit: usize,
) -> Vec<Document> {
    let scores: HashMap<Uuid, f64> = candidates
        .into_iter()
        .map(|candidate| (candidate.id, candidate.score))
        .collect();
    messages
        .into_iter()
        .filter(|message| !excluded_message_ids.contains(&message.id))
        .take(limit)
        .enumerate()
        .map(|(index, message)| {
            let mut metadata = HashMap::new();
            metadata.insert("source".into(), json!(format!("S{}", index + 1)));
            metadata.insert("message_id".into(), json!(message.id));
            metadata.insert("room_id".into(), json!(room_id));
            metadata.insert("sender".into(), json!(message.sender));
            metadata.insert("sent_at".into(), json!(message.created_at.to_rfc3339()));
            Document::new(truncate_chars(&message.content, MAX_DOCUMENT_CHARS))
                .with_metadata(metadata)
                .with_score(scores.get(&message.id).copied().unwrap_or_default())
        })
        .collect()
}

fn render_rag_context(documents: Vec<Document>) -> anyhow::Result<RagContext> {
    let mut evidence: Vec<Evidence> = documents
        .iter()
        .filter_map(Evidence::from_document)
        .collect();
    let encoded = loop {
        let encoded = toon_format::encode_default(&evidence).context("encode RAG evidence")?;
        if encoded.len() <= MAX_RAG_CONTEXT_BYTES || evidence.is_empty() {
            break encoded;
        }
        evidence.pop();
    };
    if evidence.is_empty() {
        return Ok(RagContext {
            toon_context: String::new(),
            message_count: 0,
        });
    }
    let template = PromptTemplate::new(
        RAG_CONTEXT_TEMPLATE.into(),
        vec!["evidence".into()],
        TemplateFormat::FString,
    );
    let mut variables = HashMap::new();
    variables.insert("evidence".into(), json!(encoded));
    let toon_context = template
        .format(variables)
        .map_err(|error| anyhow::anyhow!("format RAG context: {error}"))?;
    Ok(RagContext {
        toon_context,
        message_count: evidence.len(),
    })
}

#[derive(Serialize)]
struct Evidence {
    source: String,
    message_id: String,
    score: f64,
    sent_at: String,
    sender: String,
    content: String,
}

impl Evidence {
    fn from_document(document: &Document) -> Option<Self> {
        Some(Self {
            source: metadata_string(document, "source")?,
            message_id: metadata_string(document, "message_id")?,
            score: (document.score * 1_000.0).round() / 1_000.0,
            sent_at: metadata_string(document, "sent_at")?,
            sender: metadata_string(document, "sender")?,
            content: document.page_content.clone(),
        })
    }
}

fn metadata_string(document: &Document, key: &str) -> Option<String> {
    document.metadata.get(key).and_then(|value| match value {
        serde_json::Value::String(value) => Some(value.clone()),
        value if !value.is_null() => Some(value.to_string()),
        _ => None,
    })
}

fn truncate_chars(value: &str, limit: usize) -> String {
    if value.chars().count() <= limit {
        return value.to_owned();
    }
    let mut truncated: String = value.chars().take(limit.saturating_sub(1)).collect();
    truncated.push('…');
    truncated
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};

    use super::*;

    #[test]
    fn langchain_documents_preserve_scores_sources_and_exclude_recent_messages() {
        let room_id = Uuid::new_v4();
        let recent_id = Uuid::new_v4();
        let historic_id = Uuid::new_v4();
        let messages = vec![
            message(recent_id, "recent duplicate"),
            message(historic_id, "historic release decision"),
        ];
        let candidates = vec![
            ScoredMessageId {
                id: recent_id,
                score: 0.94,
            },
            ScoredMessageId {
                id: historic_id,
                score: 0.87,
            },
        ];

        let documents = documents_from_messages(
            room_id,
            candidates,
            messages,
            &HashSet::from([recent_id]),
            6,
        );

        assert_eq!(documents.len(), 1);
        assert_eq!(documents[0].page_content, "historic release decision");
        assert_eq!(documents[0].score, 0.87);
        assert_eq!(documents[0].metadata["source"], "S1");
        assert_eq!(documents[0].metadata["message_id"], historic_id.to_string());
        assert_eq!(documents[0].metadata["room_id"], room_id.to_string());
    }

    #[test]
    fn rag_context_is_structured_for_source_citations() {
        let id = Uuid::new_v4();
        let documents = documents_from_messages(
            Uuid::new_v4(),
            vec![ScoredMessageId { id, score: 0.8764 }],
            vec![message(id, "The launch date is Friday")],
            &HashSet::new(),
            6,
        );

        let context = render_rag_context(documents).unwrap();

        assert_eq!(context.message_count, 1);
        assert!(context.toon_context.contains("retrieved_evidence"));
        assert!(context.toon_context.contains("S1"));
        assert!(context.toon_context.contains("0.876"));
        assert!(context.toon_context.contains("The launch date is Friday"));
    }

    fn message(id: Uuid, content: &str) -> RetrievedMessage {
        RetrievedMessage {
            id,
            sender: "Ada".into(),
            content: content.into(),
            created_at: Utc.with_ymd_and_hms(2026, 8, 25, 10, 0, 0).unwrap(),
        }
    }
}
