use std::collections::{HashMap, HashSet};
use std::sync::atomic::Ordering;
use std::time::Duration;

use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::execution_store::{ExtractionMessage, ValidatedCandidate};
use crate::{
    ai::extraction::{AiExtractedCandidate, AiExtractionContextMessage},
    state::SharedState,
};

const DISPATCH_INTERVAL: Duration = Duration::from_secs(5);
const MAX_CANDIDATES: usize = 20;

pub(crate) fn ensure_dispatcher(state: SharedState) {
    if state
        .ai_extraction_dispatcher_started
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return;
    }
    tokio::spawn(async move {
        loop {
            if !state.maintenance_active() {
                match state.dispatchable_extraction_runs().await {
                    Ok(run_ids) => run_ids
                        .into_iter()
                        .for_each(|run_id| spawn_run(state.clone(), run_id)),
                    Err(_) => tracing::error!("dispatch durable AI extraction runs failed"),
                }
            }
            tokio::time::sleep(DISPATCH_INTERVAL).await;
        }
    });
}

pub(super) fn spawn_run(state: SharedState, run_id: Uuid) {
    tokio::spawn(async move {
        if let Err(message) = execute_run(&state, run_id).await {
            tracing::warn!(%run_id, "AI extraction run failed");
            if state.fail_extraction_run(run_id, message).await.is_err() {
                tracing::error!(%run_id, "persist AI extraction failure failed");
            }
        }
    });
}

async fn execute_run(state: &SharedState, run_id: Uuid) -> Result<(), &'static str> {
    if !state
        .claim_extraction_run(run_id)
        .await
        .map_err(|_| "AI 提取当前不可用，请稍后重试")?
    {
        return Ok(());
    }
    let execution = state
        .extraction_execution(run_id)
        .await
        .map_err(|_| "AI 提取当前不可用，请稍后重试")?
        .ok_or("聊天室已不可用")?;
    let messages = state
        .extraction_messages(&execution)
        .await
        .map_err(|_| "没有可提取的消息或你已无权访问")?;
    if messages.is_empty() {
        return state
            .complete_extraction_run(&execution, 0, &[])
            .await
            .map_err(|_| "保存提取结果失败");
    }
    let input = extraction_context(&messages);
    let assistant = execution
        .assistant(&state.config.ai)
        .ok_or("所选 AI 模型当前不可用")?;
    let raw = assistant
        .extract_decisions_and_tasks(&execution.room_name, &input)
        .await
        .map_err(|_| "AI 提取当前不可用，请稍后重试")?;
    let candidates = validate_candidates(raw, &messages).map_err(|_| "AI 返回结果无效，请重试")?;
    state
        .complete_extraction_run(&execution, messages.len() as i64, &candidates)
        .await
        .map_err(|_| "保存提取结果失败")
}

fn extraction_context(messages: &[ExtractionMessage]) -> Vec<AiExtractionContextMessage> {
    messages
        .iter()
        .enumerate()
        .map(|(index, message)| AiExtractionContextMessage {
            label: format!("S{}", index + 1),
            sent_at: message.created_at.to_rfc3339(),
            sender: message.sender.clone(),
            content: message.content.clone(),
            attachment: message.attachment.clone().unwrap_or_default(),
        })
        .collect()
}

fn validate_candidates(
    raw: Vec<AiExtractedCandidate>,
    messages: &[ExtractionMessage],
) -> anyhow::Result<Vec<ValidatedCandidate>> {
    if raw.len() > MAX_CANDIDATES {
        anyhow::bail!("too many candidates");
    }
    let labels: HashMap<String, Uuid> = messages
        .iter()
        .enumerate()
        .map(|(index, message)| (format!("S{}", index + 1), message.id))
        .collect();
    let mut seen = HashSet::new();
    let mut validated = Vec::with_capacity(raw.len());
    for candidate in raw {
        if !matches!(candidate.kind.as_str(), "decision" | "task") {
            anyhow::bail!("invalid candidate kind");
        }
        let title = normalize_spaces(&candidate.title);
        let detail = candidate.detail.trim().to_owned();
        if title.is_empty()
            || title.chars().count() > 120
            || detail.chars().count() > 2_000
            || has_unsafe_control(&title)
            || has_unsafe_control(&detail)
        {
            anyhow::bail!("invalid candidate text");
        }
        let mut source_ids = Vec::new();
        for label in candidate.source_labels {
            let id = labels
                .get(label.trim())
                .copied()
                .ok_or_else(|| anyhow::anyhow!("unknown source label"))?;
            if !source_ids.contains(&id) {
                source_ids.push(id);
            }
        }
        source_ids.sort_unstable();
        let dedupe_key = candidate_key(&candidate.kind, &title, &source_ids);
        if seen.insert(dedupe_key.clone()) {
            validated.push(ValidatedCandidate {
                kind: candidate.kind,
                title,
                detail,
                inferred: source_ids.is_empty(),
                dedupe_key,
                source_ids,
            });
        }
    }
    Ok(validated)
}

fn candidate_key(kind: &str, title: &str, source_ids: &[Uuid]) -> String {
    let mut hash = Sha256::new();
    hash.update(kind.as_bytes());
    hash.update([0]);
    hash.update(title.to_lowercase().as_bytes());
    for id in source_ids {
        hash.update(id.as_bytes());
    }
    format!("{:x}", hash.finalize())
}

fn normalize_spaces(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn has_unsafe_control(value: &str) -> bool {
    value
        .chars()
        .any(|character| character.is_control() && character != '\n' && character != '\t')
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::*;

    fn message(id: Uuid) -> ExtractionMessage {
        ExtractionMessage {
            id,
            sender: "Ada".into(),
            content: "ship it".into(),
            attachment: None,
            created_at: Utc::now(),
        }
    }

    #[test]
    fn rejects_unknown_sources_instead_of_persisting_them() {
        let result = validate_candidates(
            vec![AiExtractedCandidate {
                kind: "task".into(),
                title: "Ship".into(),
                detail: String::new(),
                source_labels: vec!["S2".into()],
            }],
            &[message(Uuid::new_v4())],
        );
        assert!(result.is_err());
    }

    #[test]
    fn duplicate_candidates_share_the_same_source_set_key() {
        let source = message(Uuid::new_v4());
        let raw = (0..2)
            .map(|_| AiExtractedCandidate {
                kind: "decision".into(),
                title: "  Ship   Friday ".into(),
                detail: "Approved".into(),
                source_labels: vec!["S1".into(), "S1".into()],
            })
            .collect();
        let result = validate_candidates(raw, &[source]).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].title, "Ship Friday");
    }
}
