//! Authorized semantic retrieval of chat messages, ranked for AI answering.
//!
//! Two responsibilities were split out: this file fetches candidates from the
//! vector index, re-authorizes them, and ranks them; `rag_evidence.rs` turns the
//! survivors into the cited TOON evidence block.

use std::collections::{HashMap, HashSet};
use std::error::Error;

use anyhow::Context;
use async_trait::async_trait;
use langchain_rust::schemas::{Document, Retriever};
use serde_json::json;
use uuid::Uuid;

use super::client::ScoredMessageId;
use super::{MessageIndex, RetrievedMessage};
use crate::state::SharedState;

#[path = "rag_evidence.rs"]
mod evidence;

use self::evidence::{render_rag_context, truncate_chars, RagContext, MAX_DOCUMENT_CHARS};

pub(crate) async fn retrieve_chat_context(
    state: SharedState,
    index: MessageIndex,
    user_id: Uuid,
    room_id: Uuid,
    question: &str,
    excluded_message_ids: HashSet<Uuid>,
    source_offset: usize,
) -> anyhow::Result<RagContext> {
    let retriever = ChatMessageRetriever {
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

struct ChatMessageRetriever {
    state: SharedState,
    index: MessageIndex,
    user_id: Uuid,
    room_id: Uuid,
    excluded_message_ids: HashSet<Uuid>,
    source_offset: usize,
}

#[async_trait]
impl Retriever for ChatMessageRetriever {
    async fn get_relevant_documents(&self, query: &str) -> Result<Vec<Document>, Box<dyn Error>> {
        self.retrieve(query).await.map_err(|error| {
            Box::new(std::io::Error::other(format!("{error:#}"))) as Box<dyn Error>
        })
    }
}

impl ChatMessageRetriever {
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
            .context("authorize retrieved chat messages")?;
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

#[cfg(test)]
#[path = "rag_tests.rs"]
mod tests;
