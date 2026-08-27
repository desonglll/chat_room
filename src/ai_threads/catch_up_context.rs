use std::collections::HashSet;

use crate::{
    ai::{bounded_conversation_context_to_toon, AiContextMessage},
    models::StoredMessage,
    state::SharedState,
};

use super::{
    context::GenerationContext,
    models::{AiCitationAttachment, AiCitationSource},
    run_store::AiRunExecution,
};

const MAX_CONTEXT_MESSAGE_CHARS: usize = 1_500;
const MAX_CONTEXT_TOON_BYTES: usize = 256 * 1024;

pub(super) async fn prepare_catch_up_context(
    state: &SharedState,
    execution: &AiRunExecution,
) -> anyhow::Result<GenerationContext> {
    let room_id = execution
        .room_id
        .ok_or_else(|| anyhow::anyhow!("catch-up run has no room"))?;
    let through = execution
        .source_through_message_id
        .ok_or_else(|| anyhow::anyhow!("catch-up run has no end boundary"))?;
    let conversation = state
        .conversation_summary(execution.user_id, room_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("active room membership is required"))?;
    let messages = state
        .catch_up_messages(
            room_id,
            execution.user_id,
            execution.source_after_message_id,
            through,
            state.ai_analysis_context_messages(),
        )
        .await?;
    build_context(room_id, &conversation.title, messages)
}

fn build_context(
    room_id: uuid::Uuid,
    room_title: &str,
    messages: Vec<StoredMessage>,
) -> anyhow::Result<GenerationContext> {
    let mut sources = Vec::with_capacity(messages.len());
    let mut context = messages
        .into_iter()
        .enumerate()
        .map(|(index, message)| {
            let label = format!("S{}", index + 1);
            sources.push(source_from_message(room_id, &label, &message));
            let attachment = message
                .attachment
                .as_ref()
                .map_or_else(String::new, |file| {
                    format!(
                        "{} ({}, {} bytes)",
                        file.file_name, file.mime_type, file.size_bytes
                    )
                });
            AiContextMessage {
                message_id: message.id.to_string(),
                sent_at: message.created_at.to_rfc3339(),
                sender: message.sender,
                content: truncate_chars(&message.content, MAX_CONTEXT_MESSAGE_CHARS),
                source: label,
                attachment,
            }
        })
        .collect::<Vec<_>>();
    let toon_context =
        bounded_conversation_context_to_toon(room_title, &mut context, MAX_CONTEXT_TOON_BYTES)?;
    let active_labels = context
        .iter()
        .map(|message| message.source.as_str())
        .collect::<HashSet<_>>();
    sources.retain(|source| active_labels.contains(source.label.as_str()));
    Ok(GenerationContext {
        history: Vec::new(),
        toon_context: (!context.is_empty()).then_some(toon_context),
        message_count: context.len() as i64,
        retrieved_message_count: 0,
        sources,
    })
}

fn source_from_message(
    room_id: uuid::Uuid,
    label: &str,
    message: &StoredMessage,
) -> AiCitationSource {
    AiCitationSource {
        label: label.into(),
        room_id,
        message_id: message.id,
        sender: message.sender.clone(),
        sent_at: message.created_at,
        excerpt: message.attachment.as_ref().map_or_else(
            || truncate_chars(&message.content, 280),
            |attachment| {
                if message.content.trim().is_empty() {
                    attachment.file_name.clone()
                } else {
                    truncate_chars(&message.content, 280)
                }
            },
        ),
        score: None,
        score_kind: "context".into(),
        attachment: message
            .attachment
            .as_ref()
            .map(|attachment| AiCitationAttachment {
                id: attachment.id,
                file_name: attachment.file_name.clone(),
                mime_type: attachment.mime_type.clone(),
                size_bytes: attachment.size_bytes,
                download_url: attachment.download_url.clone(),
                is_sensitive: attachment.is_sensitive,
            }),
    }
}

fn truncate_chars(value: &str, limit: usize) -> String {
    if value.chars().count() <= limit {
        return value.to_owned();
    }
    let mut truncated = value
        .chars()
        .take(limit.saturating_sub(1))
        .collect::<String>();
    truncated.push('…');
    truncated
}
