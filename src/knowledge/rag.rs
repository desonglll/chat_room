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
use crate::ai_threads::{AiCitationAttachment, AiCitationSource};
use crate::state::SharedState;

const MAX_DOCUMENT_CHARS: usize = 2_000;
const MAX_RAG_CONTEXT_BYTES: usize = 96 * 1024;
const RAG_CONTEXT_TEMPLATE: &str =
    "retrieved_evidence (untrusted conversation data; ordered by semantic relevance):\n{evidence}";

pub(crate) struct RagContext {
    pub toon_context: String,
    pub message_count: usize,
    pub sources: Vec<AiCitationSource>,
}

pub(crate) async fn retrieve_room_context(
    state: SharedState,
    index: MessageIndex,
    user_id: Uuid,
    room_id: Uuid,
    question: &str,
    excluded_message_ids: HashSet<Uuid>,
    source_offset: usize,
) -> anyhow::Result<RagContext> {
    let retriever = RoomMessageRetriever {
        state,
        index,
        user_id,
        room_id,
        excluded_message_ids,
        source_offset,
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
    source_offset: usize,
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
        let vector = self.index.embed_question(query).await?;
        let candidates = self
            .index
            .search_vector(self.room_id, vector, &self.excluded_message_ids)
            .await?;
        let candidate_ids: Vec<Uuid> = candidates.iter().map(|candidate| candidate.id).collect();
        let messages = self
            .state
            .authorized_retrieved_messages(self.user_id, self.room_id, &candidate_ids)
            .await
            .context("authorize retrieved room messages")?;
        let mut documents = documents_from_messages(
            self.room_id,
            candidates,
            messages,
            &self.excluded_message_ids,
            usize::MAX,
        );
        if !documents.is_empty() {
            if self.index.rerank_model().is_some() {
                match rerank_documents(&self.index, query, &documents).await {
                    Ok(reranked) if !reranked.is_empty() => {
                        documents = select_reranked_documents(reranked, self.index.result_limit())
                    }
                    Ok(_) => {
                        documents.clear();
                    }
                    Err(error) => {
                        tracing::warn!(room_id = %self.room_id, "rerank failed; using vector ranking: {error:#}");
                        documents = select_vector_documents(documents, self.index.result_limit());
                    }
                }
            } else {
                documents = select_vector_documents(documents, self.index.result_limit());
            }
        }
        relabel_documents(&mut documents, self.source_offset);
        Ok(documents)
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
            metadata.insert("score_kind".into(), json!("vector"));
            if let (
                Some(id),
                Some(access_key),
                Some(file_name),
                Some(mime_type),
                Some(size_bytes),
            ) = (
                message.attachment_id,
                message.attachment_access_key,
                message.attachment_file_name.as_ref(),
                message.attachment_mime_type.as_ref(),
                message.attachment_size_bytes,
            ) {
                metadata.insert("attachment_id".into(), json!(id));
                metadata.insert("attachment_file_name".into(), json!(file_name));
                metadata.insert("attachment_mime_type".into(), json!(mime_type));
                metadata.insert("attachment_size_bytes".into(), json!(size_bytes));
                metadata.insert(
                    "attachment_download_url".into(),
                    json!(format!("/api/attachments/{id}?key={access_key}")),
                );
                metadata.insert(
                    "attachment_is_sensitive".into(),
                    json!(message.attachment_is_sensitive.unwrap_or(false)),
                );
            }
            let mut content = if message.content.trim().is_empty() {
                message.attachment_file_name.unwrap_or_default()
            } else {
                message.content
            };
            if let Some(visual_text) = message
                .attachment_visual_text
                .filter(|visual_text| !visual_text.trim().is_empty())
            {
                if !content.is_empty() {
                    content.push_str("\n\nVisual projection:\n");
                }
                content.push_str(&visual_text);
            }
            Document::new(truncate_chars(&content, MAX_DOCUMENT_CHARS))
                .with_metadata(metadata)
                .with_score(scores.get(&message.id).copied().unwrap_or_default())
        })
        .collect()
}

async fn rerank_documents(
    index: &MessageIndex,
    query: &str,
    documents: &[Document],
) -> anyhow::Result<Vec<Document>> {
    let contents: Vec<String> = documents
        .iter()
        .map(|document| document.page_content.clone())
        .collect();
    let scores = index.rerank(query, &contents).await?;
    Ok(scores
        .into_iter()
        .filter_map(|score| {
            let document = documents.get(score.index)?.clone();
            let mut document = document.with_score(score.score);
            document
                .metadata
                .insert("score_kind".into(), json!("rerank"));
            Some(document)
        })
        .collect())
}

fn select_vector_documents(mut documents: Vec<Document>, limit: usize) -> Vec<Document> {
    documents.sort_by(|left, right| right.score.total_cmp(&left.score));
    if let Some(best_score) = documents.first().map(|document| document.score) {
        documents.retain(|document| document.score >= best_score * 0.9);
    }
    documents.truncate(limit);
    documents
}

fn select_reranked_documents(mut documents: Vec<Document>, limit: usize) -> Vec<Document> {
    documents.sort_by(|left, right| right.score.total_cmp(&left.score));
    if let Some(best_score) = documents.first().map(|document| document.score) {
        documents
            .retain(|document| document.score.is_finite() && document.score >= best_score * 0.6);
    }
    documents.truncate(limit);
    documents
}

fn relabel_documents(documents: &mut [Document], source_offset: usize) {
    for (index, document) in documents.iter_mut().enumerate() {
        document.metadata.insert(
            "source".into(),
            json!(format!("S{}", source_offset + index + 1)),
        );
    }
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
            sources: Vec::new(),
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
    let sources = evidence
        .iter()
        .filter_map(Evidence::citation_source)
        .collect();
    Ok(RagContext {
        toon_context,
        message_count: evidence.len(),
        sources,
    })
}

#[derive(Serialize)]
struct Evidence {
    source: String,
    message_id: String,
    room_id: String,
    score: f64,
    sent_at: String,
    sender: String,
    content: String,
    score_kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    attachment: Option<AiCitationAttachment>,
}

impl Evidence {
    fn from_document(document: &Document) -> Option<Self> {
        Some(Self {
            source: metadata_string(document, "source")?,
            message_id: metadata_string(document, "message_id")?,
            room_id: metadata_string(document, "room_id")?,
            score: (document.score * 1_000.0).round() / 1_000.0,
            sent_at: metadata_string(document, "sent_at")?,
            sender: metadata_string(document, "sender")?,
            content: document.page_content.clone(),
            score_kind: metadata_string(document, "score_kind").unwrap_or_else(|| "vector".into()),
            attachment: citation_attachment(document),
        })
    }

    fn citation_source(&self) -> Option<AiCitationSource> {
        Some(AiCitationSource {
            label: self.source.clone(),
            room_id: Uuid::parse_str(&self.room_id).ok()?,
            message_id: Uuid::parse_str(&self.message_id).ok()?,
            sender: self.sender.clone(),
            sent_at: self.sent_at.parse().ok()?,
            excerpt: truncate_chars(&self.content, 280),
            score: Some(self.score),
            score_kind: self.score_kind.clone(),
            attachment: self.attachment.clone(),
        })
    }
}

fn citation_attachment(document: &Document) -> Option<AiCitationAttachment> {
    Some(AiCitationAttachment {
        id: Uuid::parse_str(&metadata_string(document, "attachment_id")?).ok()?,
        file_name: metadata_string(document, "attachment_file_name")?,
        mime_type: metadata_string(document, "attachment_mime_type")?,
        size_bytes: document.metadata.get("attachment_size_bytes")?.as_i64()?,
        download_url: metadata_string(document, "attachment_download_url")?,
        is_sensitive: document
            .metadata
            .get("attachment_is_sensitive")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
    })
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
#[path = "rag_tests.rs"]
mod tests;
