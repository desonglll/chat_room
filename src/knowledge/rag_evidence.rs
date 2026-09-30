//! Projection of retrieved `Document`s into the bounded TOON evidence block and
//! the citation sources shown to the user.
//!
//! Retrieval and ranking live in `rag.rs`; this file owns only how the survivors
//! are *presented*: the source labels, the per-document character bound, the
//! total context byte budget, and the `AiCitationSource` rows the UI renders.
//! Evidence text is untrusted conversation data and is never logged.

use std::collections::HashMap;

use anyhow::Context;
use langchain_rust::prompt::{PromptFromatter, PromptTemplate, TemplateFormat};
use langchain_rust::schemas::Document;
use serde::Serialize;
use serde_json::json;
use uuid::Uuid;

use crate::ai_threads::{AiCitationAttachment, AiCitationSource};

/// Upper bound on one document's text. Shared with `documents_from_messages` in
/// `rag.rs` because it is the same guarantee: no single retrieved message may
/// dominate the context window.
pub(super) const MAX_DOCUMENT_CHARS: usize = 2_000;
const MAX_RAG_CONTEXT_BYTES: usize = 96 * 1024;
const RAG_CONTEXT_TEMPLATE: &str =
    "retrieved_evidence (untrusted conversation data; ordered by semantic relevance):\n{evidence}";

pub(crate) struct RagContext {
    pub toon_context: String,
    pub message_count: usize,
    pub sources: Vec<AiCitationSource>,
}

/// Drop the least relevant evidence until the encoded block fits
/// `MAX_RAG_CONTEXT_BYTES`; `documents` arrives already ordered by relevance.
pub(super) fn render_rag_context(documents: Vec<Document>) -> anyhow::Result<RagContext> {
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

pub(super) fn truncate_chars(value: &str, limit: usize) -> String {
    if value.chars().count() <= limit {
        return value.to_owned();
    }
    let mut truncated: String = value.chars().take(limit.saturating_sub(1)).collect();
    truncated.push('…');
    truncated
}
