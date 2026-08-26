use std::sync::atomic::Ordering;
use std::time::Duration;

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use uuid::Uuid;

use super::handlers::{current_user, internal_error, validate_room_access, DEFAULT_TITLE};
use super::models::{AiRun, AiRunTraceStep, CreateAiRunRequest};
use super::pipeline::generate_answer;
use super::run_store::{AiRunExecution, CreateRunOutcome, FailedAiRun};
use crate::ai_handlers::room_context_for_user;
use crate::cache::CachedAiAnswer;
use crate::state::SharedState;

const MAX_QUESTION_CHARS: usize = 4_000;
const DISPATCH_INTERVAL: Duration = Duration::from_secs(5);

#[utoipa::path(
    post,
    path = "/api/ai/threads/{id}/runs",
    params(("id" = Uuid, Path, description = "AI session id")),
    request_body = CreateAiRunRequest,
    responses(
        (status = 202, description = "Durable AI run accepted", body = AiRun),
        (status = 400, description = "Invalid question"),
        (status = 404, description = "AI session not found"),
        (status = 409, description = "The session already has an active run"),
        (status = 503, description = "AI assistant is unavailable")
    )
)]
pub async fn create_run(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(thread_id): Path<Uuid>,
    Json(payload): Json<CreateAiRunRequest>,
) -> Result<(StatusCode, Json<AiRun>), StatusCode> {
    let user = current_user(&state, &headers).await?;
    let mut thread = state
        .ai_thread(user.id, thread_id)
        .await
        .map_err(internal_error)?
        .ok_or(StatusCode::NOT_FOUND)?;
    let question = payload.question.trim();
    if question.is_empty() || question.chars().count() > MAX_QUESTION_CHARS {
        return Err(StatusCode::BAD_REQUEST);
    }
    let selected_model = state
        .resolve_ai_model(payload.model_option_id, thread.thinking_enabled)
        .await
        .map_err(internal_error)?
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    if !state
        .check_action_cooldown(thread_id, user.id, thread_id, state.ai_suggest_cooldown())
        .await
    {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }
    if let Some(room_id) = payload.room_id {
        validate_room_access(&state, user.id, Some(room_id)).await?;
        thread = state
            .update_ai_thread(
                user.id,
                thread.id,
                &thread.title,
                Some(room_id),
                thread.thinking_enabled,
            )
            .await
            .map_err(internal_error)?
            .ok_or(StatusCode::NOT_FOUND)?;
    }
    let room_id = payload.room_id.or(thread.room_id);
    if let Some(room_id) = room_id {
        room_context_for_user(&state, user.id, room_id, &headers).await?;
    }
    if thread.title == DEFAULT_TITLE {
        let title = title_from_question(question);
        thread = state
            .update_ai_thread(
                user.id,
                thread.id,
                &title,
                thread.room_id,
                thread.thinking_enabled,
            )
            .await
            .map_err(internal_error)?
            .ok_or(StatusCode::NOT_FOUND)?;
    }
    let outcome = state
        .create_ai_run(
            user.id,
            thread.id,
            question,
            room_id,
            payload.client_request_id,
            &selected_model,
        )
        .await
        .map_err(internal_error)?;
    let run = match outcome {
        CreateRunOutcome::Created(run) | CreateRunOutcome::Existing(run) => run,
        CreateRunOutcome::Busy => return Err(StatusCode::CONFLICT),
    };
    spawn_run(state, run.id);
    Ok((StatusCode::ACCEPTED, Json(run)))
}

#[utoipa::path(
    get,
    path = "/api/ai/runs/{id}",
    params(("id" = Uuid, Path, description = "AI run id")),
    responses(
        (status = 200, description = "AI run status", body = AiRun),
        (status = 404, description = "AI run not found")
    )
)]
pub async fn get_run(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(run_id): Path<Uuid>,
) -> Result<Json<AiRun>, StatusCode> {
    let user = current_user(&state, &headers).await?;
    state
        .ai_run(user.id, run_id)
        .await
        .map_err(internal_error)?
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

pub fn ensure_dispatcher(state: SharedState) {
    if state
        .ai_run_dispatcher_started
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return;
    }
    tokio::spawn(async move {
        loop {
            if state.maintenance_active() {
                tokio::time::sleep(DISPATCH_INTERVAL).await;
                continue;
            }
            match state.dispatchable_ai_runs().await {
                Ok(run_ids) => {
                    for run_id in run_ids {
                        spawn_run(state.clone(), run_id);
                    }
                }
                Err(error) => tracing::error!("dispatch durable AI runs failed: {error}"),
            }
            tokio::time::sleep(DISPATCH_INTERVAL).await;
        }
    });
}

fn spawn_run(state: SharedState, run_id: Uuid) {
    tokio::spawn(async move {
        if let Err(error) = execute_run(state, run_id).await {
            tracing::error!(%run_id, "execute durable AI run failed: {error:#}");
        }
    });
}

async fn execute_run(state: SharedState, run_id: Uuid) -> anyhow::Result<()> {
    if !state.claim_ai_run(run_id).await? {
        return Ok(());
    }
    let execution = state
        .ai_run_execution(run_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("AI run disappeared after claim"))?;
    if let Err(error) = generate_answer(&state, &execution).await {
        tracing::error!(%run_id, "AI run generation failed: {error:#}");
        let mut partial = live_or_persisted_answer(&state, &execution).await;
        let failed_at = chrono::Utc::now();
        if let Some(step) = partial.trace.last_mut() {
            step.completed_at.get_or_insert(failed_at);
        }
        partial.trace.push(AiRunTraceStep {
            key: "run_failed".into(),
            label: "请求失败".into(),
            detail: "AI 助手当前不可用，请稍后重试".into(),
            started_at: failed_at,
            completed_at: Some(failed_at),
        });
        state
            .fail_ai_run(
                &execution,
                FailedAiRun {
                    content: &partial.content,
                    context_message_count: partial.context_message_count,
                    retrieved_message_count: partial.retrieved_message_count,
                    sources: &partial.sources,
                    trace: &partial.trace,
                    error_message: "AI 助手当前不可用，请稍后重试",
                },
            )
            .await?;
        cache_terminal_answer(
            &state,
            &execution,
            CachedAiAnswer {
                content: partial.content,
                context_message_count: partial.context_message_count,
                retrieved_message_count: partial.retrieved_message_count,
                sources: partial.sources,
                trace: partial.trace,
                revision: partial.revision + 1,
                status: "failed".into(),
                stage: "failed".into(),
                stage_started_at: Some(chrono::Utc::now()),
                updated_at: chrono::Utc::now(),
            },
        )
        .await;
    }
    Ok(())
}

async fn live_or_persisted_answer(
    state: &SharedState,
    execution: &AiRunExecution,
) -> CachedAiAnswer {
    if let Some(cache) = state.redis_cache() {
        match cache.ai_answer(execution.assistant_message_id).await {
            Ok(Some(answer)) => return answer,
            Ok(None) => {}
            Err(error) => tracing::warn!(
                run_id = %execution.id,
                "read failed AI answer from Redis failed: {error:#}"
            ),
        }
    }
    if let Ok(Some(message)) = state.ai_run_message(execution.user_id, execution.id).await {
        return CachedAiAnswer {
            content: message.content,
            context_message_count: message.context_message_count.unwrap_or_default(),
            retrieved_message_count: message.retrieved_message_count.unwrap_or_default(),
            sources: message.sources.0,
            trace: message.trace.0,
            revision: message.revision,
            status: message.status,
            stage: message.stage,
            stage_started_at: message.stage_started_at,
            updated_at: message.updated_at,
        };
    }
    CachedAiAnswer {
        content: String::new(),
        context_message_count: 0,
        retrieved_message_count: 0,
        sources: Vec::new(),
        trace: Vec::new(),
        revision: 0,
        status: "streaming".into(),
        stage: "queued".into(),
        stage_started_at: None,
        updated_at: chrono::Utc::now(),
    }
}

pub(super) async fn cache_terminal_answer(
    state: &SharedState,
    execution: &AiRunExecution,
    answer: CachedAiAnswer,
) {
    let Some(cache) = state.redis_cache() else {
        return;
    };
    if let Err(error) = cache
        .set_ai_answer(
            execution.assistant_message_id,
            &answer,
            state.ai_answer_cache_ttl_secs(),
        )
        .await
    {
        tracing::warn!(run_id = %execution.id, "cache terminal AI answer failed: {error:#}");
    }
}

fn title_from_question(question: &str) -> String {
    let mut title: String = question.chars().take(28).collect();
    if question.chars().count() > 28 {
        title.push('…');
    }
    title
}
