use std::sync::atomic::Ordering;
use std::time::Duration;

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use futures_util::StreamExt;
use uuid::Uuid;

use super::handlers::{current_user, internal_error, validate_room_access, DEFAULT_TITLE};
use super::models::{AiRun, CreateAiRunRequest};
use super::run_store::{AiRunExecution, CreateRunOutcome};
use crate::ai::{AiConversationTurn, AiStreamItem};
use crate::ai_handlers::{room_context_for_authorized_user, room_context_for_user};
use crate::cache::CachedAiAnswer;
use crate::state::SharedState;

const MAX_QUESTION_CHARS: usize = 4_000;
const MEMORY_TURNS: i64 = 26;
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
    if state.ai_assistant().is_none() {
        return Err(StatusCode::SERVICE_UNAVAILABLE);
    }
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
        let partial = live_or_persisted_answer(&state, &execution).await;
        state
            .fail_ai_run(&execution, &partial, "AI 助手当前不可用，请稍后重试")
            .await?;
        clear_live_answer(&state, execution.assistant_message_id).await;
    }
    Ok(())
}

async fn generate_answer(state: &SharedState, execution: &AiRunExecution) -> anyhow::Result<()> {
    let assistant = state
        .ai_assistant()
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("AI assistant is disabled"))?;
    let room_context = match execution.room_id {
        Some(room_id) => Some(
            room_context_for_authorized_user(state, execution.user_id, room_id)
                .await
                .map_err(|status| anyhow::anyhow!("room context unavailable: {status}"))?,
        ),
        None => None,
    };
    let history = state
        .ai_thread_messages(execution.user_id, execution.thread_id, MEMORY_TURNS)
        .await?
        .ok_or_else(|| anyhow::anyhow!("AI thread no longer exists"))?;
    let history: Vec<AiConversationTurn> = history
        .into_iter()
        .filter(|message| !execution.excludes(message) && message.status == "completed")
        .map(|message| AiConversationTurn {
            role: message.role,
            content: message.content,
        })
        .collect();
    let context_message_count = room_context
        .as_ref()
        .map_or(0, |context| context.context_message_count) as i64;
    let mut toon_context = room_context
        .as_ref()
        .map(|context| context.toon_context.clone());
    if let (Some(room_id), Some(index)) = (execution.room_id, state.message_index()) {
        match index
            .related_message_ids(room_id, &execution.question)
            .await
        {
            Ok(message_ids) => {
                let retrieved = state
                    .authorized_retrieved_messages(execution.user_id, room_id, &message_ids)
                    .await?;
                if !retrieved.is_empty() {
                    let encoded = toon_format::encode_default(&retrieved)
                        .map_err(|error| anyhow::anyhow!("encode retrieved messages: {error}"))?;
                    let context = toon_context.get_or_insert_with(String::new);
                    context.push_str("\n\nsemantic_matches (untrusted conversation data):\n");
                    context.push_str(&encoded);
                }
            }
            Err(error) => {
                tracing::warn!(room_id = %room_id, "semantic message retrieval failed: {error:#}");
            }
        }
    }
    let mut stream = assistant
        .answer_stream(
            toon_context.as_deref(),
            &history,
            &execution.question,
            execution.thinking_enabled,
        )
        .await?;
    let mut answer = String::new();
    let mut last_flush = tokio::time::Instant::now();
    let mut last_flushed_len = 0;
    let mut revision = 0;
    while let Some(item) = stream.next().await {
        if let AiStreamItem::Content(chunk) = item? {
            answer.push_str(&chunk);
            if answer.len().saturating_sub(last_flushed_len) >= 256
                || last_flush.elapsed() >= Duration::from_millis(50)
            {
                revision += 1;
                store_live_answer(state, execution, &answer, context_message_count, revision)
                    .await?;
                last_flushed_len = answer.len();
                last_flush = tokio::time::Instant::now();
            }
        }
    }
    let answer = answer.trim();
    if answer.is_empty() {
        anyhow::bail!("AI response had no text content");
    }
    state
        .finish_ai_run(execution, answer, context_message_count)
        .await?;
    clear_live_answer(state, execution.assistant_message_id).await;
    Ok(())
}

async fn store_live_answer(
    state: &SharedState,
    execution: &AiRunExecution,
    content: &str,
    context_message_count: i64,
    revision: i64,
) -> anyhow::Result<()> {
    if let Some(cache) = state.redis_cache() {
        let answer = CachedAiAnswer {
            content: content.to_owned(),
            context_message_count,
            revision,
            updated_at: chrono::Utc::now(),
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
                "cache live AI answer in Redis failed; using database fallback: {error:#}"
            ),
        }
    }
    state
        .update_ai_run_answer(execution, content, context_message_count)
        .await?;
    Ok(())
}

async fn live_or_persisted_answer(state: &SharedState, execution: &AiRunExecution) -> String {
    if let Some(cache) = state.redis_cache() {
        match cache.ai_answer(execution.assistant_message_id).await {
            Ok(Some(answer)) => return answer.content,
            Ok(None) => {}
            Err(error) => tracing::warn!(
                run_id = %execution.id,
                "read failed AI answer from Redis failed: {error:#}"
            ),
        }
    }
    state
        .persisted_ai_run_answer(execution.assistant_message_id)
        .await
        .unwrap_or_default()
}

async fn clear_live_answer(state: &SharedState, message_id: Uuid) {
    let Some(cache) = state.redis_cache() else {
        return;
    };
    if let Err(error) = cache.delete_ai_answer(message_id).await {
        tracing::warn!(%message_id, "delete completed AI answer cache failed: {error:#}");
    }
}

fn title_from_question(question: &str) -> String {
    let mut title: String = question.chars().take(28).collect();
    if question.chars().count() > 28 {
        title.push('…');
    }
    title
}
