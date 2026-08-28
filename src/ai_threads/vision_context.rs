use futures_util::{stream, StreamExt};
use serde::Serialize;

use crate::ai::{AiAssistant, VisionLimits, VisualProjection};
use crate::state::SharedState;

use super::context::GenerationContext;
use super::models::AiCitationSource;
use super::progress::{RunProgress, RunStage, RunStep};
use super::run_store::AiRunExecution;
use super::vision_selection::select_image_sources;
use super::vision_store::{load_cached_projection, read_authorized_image, store_visual_projection};

const VISION_CONCURRENCY: usize = 3;
const MAX_VISUAL_CONTEXT_BYTES: usize = 96 * 1024;

#[derive(Serialize)]
struct VisualEvidence {
    source: String,
    message_id: String,
    sender: String,
    sent_at: String,
    nearby_message: String,
    attachment_id: String,
    attachment_file_name: String,
    projection: VisualProjection,
}

#[derive(Serialize)]
struct VisualEvidenceContext<'a> {
    source_messages: &'a [VisualEvidence],
}

enum ProjectionOutcome {
    Cached(VisualProjection),
    Generated(VisualProjection),
    Unavailable,
    CacheFailure,
    ProviderFailure,
    StorageFailure,
}

#[derive(Default)]
struct ProjectionStats {
    cache_hits: usize,
    generated: usize,
    unavailable: usize,
    cache_failures: usize,
    provider_failures: usize,
    storage_failures: usize,
}

pub(super) async fn enrich_with_visual_evidence(
    state: &SharedState,
    execution: &AiRunExecution,
    assistant: &AiAssistant,
    context: &mut GenerationContext,
    progress: &mut RunProgress,
) -> anyhow::Result<()> {
    let image_attachments = context
        .sources
        .iter()
        .filter(|source| image_attachment(source))
        .count();
    if image_attachments == 0 {
        return Ok(());
    }
    let sensitive_images = context
        .sources
        .iter()
        .filter(|source| image_is_sensitive(source))
        .count();
    let Some(limits) = assistant.vision_limits() else {
        progress
            .publish_step(
                state,
                execution,
                RunStep::new(
                    RunStage::PreparingContext,
                    "vision_skipped",
                    "跳过图片理解",
                    format!("发现 {image_attachments} 张图片；视觉模型未配置，保留附件元数据"),
                ),
                "",
            )
            .await?;
        return Ok(());
    };
    let eligible_images = context
        .sources
        .iter()
        .filter(|source| image_is_eligible(source, limits.max_image_bytes))
        .count();
    let invalid_images = image_attachments
        .saturating_sub(sensitive_images)
        .saturating_sub(eligible_images);
    let candidates = select_image_sources(
        &context.sources,
        &execution.question,
        limits.max_images,
        limits.max_total_images,
        limits.max_image_bytes,
    );
    if candidates.is_empty() {
        progress
            .publish_step(
                state,
                execution,
                RunStep::new(
                    RunStage::PreparingContext,
                    "vision_skipped",
                    "跳过图片理解",
                    format!(
                        "发现 {image_attachments} 张图片；敏感 {sensitive_images} 张；尺寸或格式不合格 {invalid_images} 张"
                    ),
                ),
                "",
            )
            .await?;
        return Ok(());
    }
    let budget_omitted = eligible_images.saturating_sub(candidates.len());
    progress
        .publish_step(
            state,
            execution,
            RunStep::new(
                RunStage::PreparingContext,
                "vision_context",
                "使用视觉模型理解图片",
                format!(
                    "发现 {image_attachments} 张图片；可处理 {eligible_images} 张；本次选择 {} 张；敏感 {sensitive_images} 张；尺寸或格式不合格 {invalid_images} 张；预算省略 {budget_omitted} 张",
                    candidates.len(),
                ),
            ),
            "",
        )
        .await?;

    let (vision_model, prompt_version) = assistant
        .vision_identity()
        .expect("vision limits and identity are configured together");
    let vision_model = vision_model.to_owned();
    let analyzed = stream::iter(candidates.into_iter().enumerate())
        .map(|(index, source)| {
            let vision_model = &vision_model;
            async move {
                let outcome = analyze_source(
                    state,
                    execution,
                    assistant,
                    &source,
                    limits,
                    vision_model,
                    prompt_version,
                )
                .await;
                (index, source, outcome)
            }
        })
        .buffer_unordered(VISION_CONCURRENCY)
        .collect::<Vec<_>>()
        .await;
    let mut analyzed = analyzed;
    analyzed.sort_by_key(|(index, _, _)| *index);
    let mut stats = ProjectionStats::default();
    let mut evidence = Vec::new();
    for (_, source, outcome) in analyzed {
        let projection = match outcome {
            ProjectionOutcome::Cached(projection) => {
                stats.cache_hits += 1;
                Some(projection)
            }
            ProjectionOutcome::Generated(projection) => {
                stats.generated += 1;
                Some(projection)
            }
            ProjectionOutcome::Unavailable => {
                stats.unavailable += 1;
                None
            }
            ProjectionOutcome::CacheFailure => {
                stats.cache_failures += 1;
                None
            }
            ProjectionOutcome::ProviderFailure => {
                stats.provider_failures += 1;
                None
            }
            ProjectionOutcome::StorageFailure => {
                stats.storage_failures += 1;
                None
            }
        };
        if let Some(projection) = projection.and_then(|projection| evidence_for(source, projection))
        {
            evidence.push(projection);
        }
    }
    let extracted = evidence.len();
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
                format!(
                    "选择 {} 张；缓存命中 {} 张；新提取 {} 张；不可用 {} 张；缓存失败 {} 张；模型失败 {} 张；存储失败 {} 张；上下文纳入 {} 张；上下文预算省略 {} 张；候选预算省略 {budget_omitted} 张",
                    extracted
                        + stats.unavailable
                        + stats.cache_failures
                        + stats.provider_failures
                        + stats.storage_failures,
                    stats.cache_hits,
                    stats.generated,
                    stats.unavailable,
                    stats.cache_failures,
                    stats.provider_failures,
                    stats.storage_failures,
                    evidence.len(),
                    extracted.saturating_sub(evidence.len()),
                ),
            ),
            "",
        )
        .await?;
    Ok(())
}

async fn analyze_source(
    state: &SharedState,
    execution: &AiRunExecution,
    assistant: &AiAssistant,
    source: &AiCitationSource,
    limits: VisionLimits,
    vision_model: &str,
    prompt_version: i64,
) -> ProjectionOutcome {
    match load_cached_projection(state, execution, source, vision_model, prompt_version).await {
        Ok(Some(projection)) => return ProjectionOutcome::Cached(projection),
        Ok(None) => {}
        Err(_) => return ProjectionOutcome::CacheFailure,
    }
    let image = match read_authorized_image(state, execution, source, limits).await {
        Ok(Some(image)) => image,
        Ok(None) | Err(_) => return ProjectionOutcome::Unavailable,
    };
    let projection = match assistant
        .describe_image(&execution.question, &source.label, &source.excerpt, image)
        .await
    {
        Ok(projection) => projection,
        Err(_) => return ProjectionOutcome::ProviderFailure,
    };
    match store_visual_projection(
        state,
        execution,
        source,
        vision_model,
        prompt_version,
        &projection,
    )
    .await
    {
        Ok(true) => ProjectionOutcome::Generated(projection),
        Ok(false) => ProjectionOutcome::Unavailable,
        Err(_) => ProjectionOutcome::StorageFailure,
    }
}

fn evidence_for(source: AiCitationSource, projection: VisualProjection) -> Option<VisualEvidence> {
    let attachment = source.attachment?;
    Some(VisualEvidence {
        source: source.label,
        message_id: source.message_id.to_string(),
        sender: source.sender,
        sent_at: source.sent_at.to_rfc3339(),
        nearby_message: source.excerpt,
        attachment_id: attachment.id.to_string(),
        attachment_file_name: attachment.file_name,
        projection,
    })
}

fn image_attachment(source: &AiCitationSource) -> bool {
    source.attachment.as_ref().is_some_and(|attachment| {
        attachment.mime_type.starts_with("image/") && attachment.size_bytes > 0
    })
}

fn image_is_sensitive(source: &AiCitationSource) -> bool {
    source.attachment.as_ref().is_some_and(|attachment| {
        attachment.mime_type.starts_with("image/") && attachment.is_sensitive
    })
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
            source_messages: evidence,
        })?;
        if encoded.len() <= MAX_VISUAL_CONTEXT_BYTES {
            return Ok(Some(encoded));
        }
        evidence.pop();
    }
    Ok(None)
}

#[cfg(test)]
#[path = "vision_context_tests.rs"]
mod tests;
