use futures_util::{stream, StreamExt};
use serde::Serialize;
use tokio::io::AsyncReadExt;

use crate::ai::{AiAssistant, VisionImage, VisionLimits};
use crate::state::{with_pool, SharedState};

use super::context::GenerationContext;
use super::models::AiCitationSource;
use super::progress::{RunProgress, RunStage, RunStep};
use super::run_store::AiRunExecution;
use super::vision_selection::select_image_sources;

const VISION_CONCURRENCY: usize = 3;
const MAX_OBSERVATION_CHARS: usize = 3_000;
const MAX_VISUAL_CONTEXT_BYTES: usize = 32 * 1024;

#[derive(Serialize)]
struct VisualEvidence {
    source: String,
    message_id: String,
    sender: String,
    sent_at: String,
    observation: String,
}

#[derive(Serialize)]
struct VisualEvidenceContext<'a> {
    visual_evidence: &'a [VisualEvidence],
}

pub(super) async fn enrich_with_visual_evidence(
    state: &SharedState,
    execution: &AiRunExecution,
    assistant: &AiAssistant,
    context: &mut GenerationContext,
    progress: &mut RunProgress,
) -> anyhow::Result<()> {
    let eligible_images = context
        .sources
        .iter()
        .filter(|source| image_is_eligible(source, u64::MAX))
        .count();
    if eligible_images == 0 {
        return Ok(());
    }
    let Some(limits) = assistant.vision_limits() else {
        progress
            .publish_step(
                state,
                execution,
                RunStep::new(
                    RunStage::PreparingContext,
                    "vision_skipped",
                    "跳过图片理解",
                    "视觉模型未配置，保留经过授权的附件元数据",
                ),
                "",
            )
            .await?;
        return Ok(());
    };
    let candidates = select_image_sources(
        &context.sources,
        &execution.question,
        limits.max_images,
        limits.max_image_bytes,
    );
    if candidates.is_empty() {
        return Ok(());
    }
    progress
        .publish_step(
            state,
            execution,
            RunStep::new(
                RunStage::PreparingContext,
                "vision_context",
                "使用视觉模型理解图片",
                format!(
                    "从 {eligible_images} 张授权图片中选择 {} 张进行 OCR 与视觉内容提取",
                    candidates.len()
                ),
            ),
            "",
        )
        .await?;

    let analyzed = stream::iter(candidates.into_iter().enumerate())
        .map(|(index, source)| async move {
            let image = read_authorized_image(state, execution, &source, limits).await;
            let result = match image {
                Ok(Some(image)) => {
                    assistant
                        .describe_image(&execution.question, &source.label, &source.excerpt, image)
                        .await
                }
                Ok(None) => return (index, source, None),
                Err(_) => return (index, source, None),
            };
            (index, source, result.ok())
        })
        .buffer_unordered(VISION_CONCURRENCY)
        .collect::<Vec<_>>()
        .await;
    let mut analyzed = analyzed;
    analyzed.sort_by_key(|(index, _, _)| *index);
    let mut evidence = analyzed
        .into_iter()
        .filter_map(|(_, source, observation)| {
            observation.map(|observation| VisualEvidence {
                source: source.label,
                message_id: source.message_id.to_string(),
                sender: source.sender,
                sent_at: source.sent_at.to_rfc3339(),
                observation: truncate_chars(&observation, MAX_OBSERVATION_CHARS),
            })
        })
        .collect::<Vec<_>>();
    let encoded = bounded_visual_context(&mut evidence)?;
    if let Some(encoded) = encoded {
        context
            .toon_context
            .get_or_insert_with(String::new)
            .push_str(&format!("\n{encoded}"));
    }
    progress
        .publish_step(
            state,
            execution,
            RunStep::new(
                RunStage::PreparingContext,
                "vision_context",
                "使用视觉模型理解图片",
                format!("已将 {} 张图片的视觉证据放回原消息上下文", evidence.len()),
            ),
            "",
        )
        .await?;
    Ok(())
}

async fn read_authorized_image(
    state: &SharedState,
    execution: &AiRunExecution,
    source: &AiCitationSource,
    limits: VisionLimits,
) -> anyhow::Result<Option<VisionImage>> {
    let Some(room_id) = execution.room_id else {
        return Ok(None);
    };
    let Some(attachment) = source.attachment.as_ref() else {
        return Ok(None);
    };
    let row: Option<(String, String, i64, Option<String>)> = with_pool!(state, |pool| {
        sqlx::query_as(
            "SELECT attachments.file_name, attachments.mime_type, attachments.size_bytes, \
             attachments.storage_key FROM attachments JOIN messages \
             ON messages.attachment_id = attachments.id \
             WHERE attachments.id = $1 AND messages.id = $2 AND messages.room_id = $3 \
               AND messages.recalled_at IS NULL AND EXISTS (SELECT 1 FROM room_memberships \
                 WHERE room_memberships.room_id = $3 AND room_memberships.user_id = $4 \
                   AND room_memberships.status = 'active') LIMIT 1",
        )
        .bind(attachment.id)
        .bind(source.message_id)
        .bind(room_id)
        .bind(execution.user_id)
        .fetch_optional(pool)
        .await
    })?;
    let Some((file_name, content_type, size_bytes, storage_key)) = row else {
        return Ok(None);
    };
    let size = u64::try_from(size_bytes)?;
    if size == 0 || size > limits.max_image_bytes || !content_type.starts_with("image/") {
        return Ok(None);
    }
    let storage_key = storage_key.unwrap_or_else(|| attachment.id.simple().to_string());
    let reader = state
        .attachment_store()
        .open_range(&storage_key, 0, size)
        .await?;
    let mut bytes = Vec::with_capacity(usize::try_from(size)?);
    reader
        .take(limits.max_image_bytes.saturating_add(1))
        .read_to_end(&mut bytes)
        .await?;
    if bytes.is_empty() || bytes.len() as u64 > limits.max_image_bytes {
        return Ok(None);
    }
    Ok(Some(VisionImage {
        content_type,
        file_name,
        bytes,
    }))
}

fn image_is_eligible(source: &AiCitationSource, max_image_bytes: u64) -> bool {
    source.attachment.as_ref().is_some_and(|attachment| {
        !attachment.is_sensitive
            && attachment.mime_type.starts_with("image/")
            && attachment.size_bytes > 0
            && u64::try_from(attachment.size_bytes).is_ok_and(|size| size <= max_image_bytes)
    })
}

fn bounded_visual_context(evidence: &mut Vec<VisualEvidence>) -> anyhow::Result<Option<String>> {
    while !evidence.is_empty() {
        let encoded = toon_format::encode_default(&VisualEvidenceContext {
            visual_evidence: evidence,
        })?;
        if encoded.len() <= MAX_VISUAL_CONTEXT_BYTES {
            return Ok(Some(encoded));
        }
        evidence.pop();
    }
    Ok(None)
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

#[cfg(test)]
#[path = "vision_context_tests.rs"]
mod tests;
