use std::time::Duration;

use futures_util::StreamExt;

use super::context::{prepare_generation_context, GenerationContext};
use super::planner::{fallback_plan, plan_request};
use super::progress::{RunProgress, RunStage, RunStep};
use super::run_store::AiRunExecution;
use super::runs::cache_terminal_answer;
use crate::ai::{AiAssistant, AiStreamItem};
use crate::cache::CachedAiAnswer;
use crate::state::SharedState;

pub(super) async fn generate_answer(
    state: &SharedState,
    execution: &AiRunExecution,
) -> anyhow::Result<()> {
    let assistant = execution
        .assistant(&state.config.ai)
        .ok_or_else(|| anyhow::anyhow!("AI assistant is disabled"))?;
    let mut progress = RunProgress::new();

    progress
        .publish_step(
            state,
            execution,
            RunStep::new(
                RunStage::PreparingContext,
                "analyze_command",
                "调用规划 Agent",
                "请求快速模型判断上下文范围、检索策略和研究子问题",
            ),
            "",
        )
        .await?;
    let has_room = execution.room_id.is_some();
    let plan = match plan_request(&assistant, &execution.question, has_room).await {
        Ok(plan) => plan,
        Err(error) => {
            tracing::warn!(run_id = %execution.id, "planning agent failed; using safe fallback: {error:#}");
            fallback_plan(has_room)
        }
    };
    progress
        .publish_step(
            state,
            execution,
            RunStep::new(
                RunStage::PreparingContext,
                "build_execution_plan",
                "确认执行计划",
                plan.detail(),
            ),
            "",
        )
        .await?;
    progress
        .publish_step(
            state,
            execution,
            RunStep::new(
                RunStage::PreparingContext,
                "load_context",
                "执行上下文工具",
                "按计划读取经过权限校验的聊天室消息和 AI 会话历史",
            ),
            "",
        )
        .await?;
    let context = prepare_generation_context(state, execution, &plan, &mut progress).await?;
    progress.set_context(
        context.message_count,
        context.retrieved_message_count,
        &context.sources,
    );
    progress
        .publish_step(
            state,
            execution,
            RunStep::new(
                RunStage::PreparingContext,
                "tool_feedback",
                "检查工具执行结果",
                format!(
                    "聊天室上下文 {} 条，历史检索证据 {} 条",
                    context.message_count - context.retrieved_message_count,
                    context.retrieved_message_count
                ),
            ),
            "",
        )
        .await?;
    progress
        .publish_step(
            state,
            execution,
            RunStep::new(
                RunStage::PreparingContext,
                "build_prompt",
                "组装最终模型输入",
                format!("执行意图：{}；已注入真实工具反馈", plan.intent_label()),
            ),
            "",
        )
        .await?;

    let answer = stream_model_answer(
        state,
        execution,
        &assistant,
        &context,
        plan.intent_label(),
        &mut progress,
    )
    .await?;
    progress.complete_current_step();
    state
        .finish_ai_run(
            execution,
            &answer,
            context.message_count,
            context.retrieved_message_count,
            &context.sources,
            progress.trace(),
        )
        .await?;
    cache_terminal_answer(
        state,
        execution,
        CachedAiAnswer {
            content: answer,
            context_message_count: context.message_count,
            retrieved_message_count: context.retrieved_message_count,
            sources: context.sources,
            trace: progress.trace().to_vec(),
            revision: progress.revision() + 1,
            status: "completed".into(),
            stage: "completed".into(),
            stage_started_at: Some(chrono::Utc::now()),
            updated_at: chrono::Utc::now(),
        },
    )
    .await;
    Ok(())
}

async fn stream_model_answer(
    state: &SharedState,
    execution: &AiRunExecution,
    assistant: &AiAssistant,
    context: &GenerationContext,
    task_label: &str,
    progress: &mut RunProgress,
) -> anyhow::Result<String> {
    progress
        .publish_step(
            state,
            execution,
            RunStep::new(
                RunStage::ConnectingModel,
                "request_model_api",
                "请求模型 API",
                execution.request_label(),
            ),
            "",
        )
        .await?;
    let mut stream = assistant
        .answer_stream(
            context.toon_context.as_deref(),
            &context.history,
            &execution.question,
            context.retrieved_message_count > 0,
            execution.thinking_enabled,
            Some(task_label),
        )
        .await?;
    progress
        .publish_step(
            state,
            execution,
            RunStep::new(
                RunStage::WaitingForModel,
                "wait_first_token",
                "等待模型首个响应",
                "请求已建立，等待上游返回第一个数据块",
            ),
            "",
        )
        .await?;
    let mut answer = String::new();
    let mut last_flush = tokio::time::Instant::now();
    let mut last_flushed_len = 0;
    while let Some(item) = stream.next().await {
        match item? {
            AiStreamItem::Reasoning if progress.stage() != RunStage::Reasoning => {
                progress
                    .publish_step(
                        state,
                        execution,
                        RunStep::new(
                            RunStage::Reasoning,
                            "model_reasoning",
                            "模型推理",
                            "已收到推理数据，等待正文",
                        ),
                        &answer,
                    )
                    .await?;
            }
            AiStreamItem::Reasoning => {}
            AiStreamItem::Content(chunk) => {
                let first_content = progress.stage() != RunStage::Responding;
                answer.push_str(&chunk);
                if first_content
                    || answer.len().saturating_sub(last_flushed_len) >= 256
                    || last_flush.elapsed() >= Duration::from_millis(50)
                {
                    progress
                        .publish_step(
                            state,
                            execution,
                            RunStep::new(
                                RunStage::Responding,
                                "stream_answer",
                                "流式生成回答",
                                format!("已接收 {} 个字符", answer.chars().count()),
                            ),
                            &answer,
                        )
                        .await?;
                    last_flushed_len = answer.len();
                    last_flush = tokio::time::Instant::now();
                }
            }
        }
    }
    let answer = answer.trim().to_owned();
    if answer.is_empty() {
        anyhow::bail!("AI response had no text content");
    }
    Ok(answer)
}
