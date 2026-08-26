use chrono::{DateTime, Utc};

use super::models::{AiCitationSource, AiRunTraceStep};
use super::run_store::{AiAnswerSnapshot, AiRunExecution};
use crate::cache::CachedAiAnswer;
use crate::state::SharedState;

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum RunStage {
    Queued,
    PreparingContext,
    RetrievingContext,
    ConnectingModel,
    WaitingForModel,
    Reasoning,
    Responding,
}

impl RunStage {
    fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::PreparingContext => "preparing_context",
            Self::RetrievingContext => "retrieving_context",
            Self::ConnectingModel => "connecting_model",
            Self::WaitingForModel => "waiting_for_model",
            Self::Reasoning => "reasoning",
            Self::Responding => "responding",
        }
    }
}

pub(super) struct RunStep {
    stage: RunStage,
    key: String,
    label: &'static str,
    detail: String,
}

impl RunStep {
    pub fn new(
        stage: RunStage,
        key: impl Into<String>,
        label: &'static str,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            stage,
            key: key.into(),
            label,
            detail: detail.into(),
        }
    }
}

pub(super) struct RunProgress {
    revision: i64,
    stage: RunStage,
    stage_started_at: DateTime<Utc>,
    context_message_count: i64,
    retrieved_message_count: i64,
    sources: Vec<AiCitationSource>,
    trace: Vec<AiRunTraceStep>,
}

impl RunProgress {
    pub fn new() -> Self {
        Self {
            revision: 0,
            stage: RunStage::Queued,
            stage_started_at: Utc::now(),
            context_message_count: 0,
            retrieved_message_count: 0,
            sources: Vec::new(),
            trace: Vec::new(),
        }
    }

    pub fn stage(&self) -> RunStage {
        self.stage
    }

    pub fn revision(&self) -> i64 {
        self.revision
    }

    pub fn trace(&self) -> &[AiRunTraceStep] {
        &self.trace
    }

    pub fn complete_current_step(&mut self) {
        if let Some(step) = self.trace.last_mut() {
            step.completed_at.get_or_insert_with(Utc::now);
        }
    }

    pub fn set_context(
        &mut self,
        message_count: i64,
        retrieved_message_count: i64,
        sources: &[AiCitationSource],
    ) {
        self.context_message_count = message_count;
        self.retrieved_message_count = retrieved_message_count;
        self.sources = sources.to_vec();
    }

    pub async fn publish_step(
        &mut self,
        state: &SharedState,
        execution: &AiRunExecution,
        step: RunStep,
        content: &str,
    ) -> anyhow::Result<()> {
        let now = Utc::now();
        if self.stage != step.stage {
            self.stage = step.stage;
            self.stage_started_at = now;
        }
        if self.trace.last().is_none_or(|last| last.key != step.key) {
            self.complete_current_step();
            self.trace.push(AiRunTraceStep {
                key: step.key,
                label: step.label.into(),
                detail: step.detail,
                started_at: now,
                completed_at: None,
            });
        } else if let Some(last) = self.trace.last_mut() {
            last.label = step.label.into();
            last.detail = step.detail;
        }
        self.revision += 1;
        let updated_at = now;
        if let Some(cache) = state.redis_cache() {
            let answer = CachedAiAnswer {
                content: content.to_owned(),
                context_message_count: self.context_message_count,
                retrieved_message_count: self.retrieved_message_count,
                sources: self.sources.clone(),
                trace: self.trace.clone(),
                revision: self.revision,
                status: "streaming".into(),
                stage: self.stage.as_str().into(),
                stage_started_at: Some(self.stage_started_at),
                updated_at,
            };
            match cache
                .set_ai_answer(
                    execution.assistant_message_id,
                    &answer,
                    state.ai_answer_cache_ttl_secs(),
                )
                .await
            {
                Ok(()) => return Ok(state.heartbeat_ai_run(execution.id).await?),
                Err(error) => tracing::warn!(
                    run_id = %execution.id,
                    "cache live AI progress in Redis failed; using database fallback: {error:#}"
                ),
            }
        }
        state
            .update_ai_run_answer(
                execution,
                &AiAnswerSnapshot {
                    content,
                    context_message_count: self.context_message_count,
                    retrieved_message_count: self.retrieved_message_count,
                    sources: &self.sources,
                    trace: &self.trace,
                    stage: self.stage.as_str(),
                    stage_started_at: self.stage_started_at,
                    revision: self.revision,
                },
            )
            .await?;
        Ok(())
    }
}
