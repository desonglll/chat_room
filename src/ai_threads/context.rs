use std::time::Duration;

use crate::ai::AiConversationTurn;
use crate::ai_handlers::room_context_for_authorized_user_with_limit;
use crate::knowledge::retrieve_room_context;
use crate::state::SharedState;
use futures_util::{stream::FuturesUnordered, StreamExt};

use super::catch_up_context::prepare_catch_up_context;
use super::models::AiCitationSource;
use super::planner::{AgentPlan, ContextScope};
use super::progress::{RunProgress, RunStage, RunStep};
use super::run_store::AiRunExecution;
use super::selected_context::prepare_selected_context;

const MEMORY_TURNS: i64 = 26;
const SEMANTIC_SEARCH_TIMEOUT: Duration = Duration::from_secs(5);

pub(super) struct GenerationContext {
    pub history: Vec<AiConversationTurn>,
    pub toon_context: Option<String>,
    pub message_count: i64,
    pub retrieved_message_count: i64,
    pub sources: Vec<AiCitationSource>,
}

pub(super) async fn prepare_generation_context(
    state: &SharedState,
    execution: &AiRunExecution,
    plan: &AgentPlan,
    progress: &mut RunProgress,
) -> anyhow::Result<GenerationContext> {
    if execution.is_catch_up() {
        return prepare_catch_up_context(state, execution).await;
    }
    let selected_message_ids = state.ai_run_selected_message_ids(execution.id).await?;
    if !selected_message_ids.is_empty() {
        let mut context = prepare_selected_context(state, execution, selected_message_ids).await?;
        context.history = completed_thread_history(state, execution).await?;
        progress
            .publish_step(
                state,
                execution,
                RunStep::new(
                    RunStage::PreparingContext,
                    "selected_message_scope",
                    "使用所选聊天记录",
                    format!(
                        "仅使用用户明确选择的 {} 条消息，不扩展房间范围",
                        context.message_count
                    ),
                ),
                "",
            )
            .await?;
        return Ok(context);
    }
    let context_limit = match plan.context_scope {
        ContextScope::None => None,
        ContextScope::Recent => Some(state.ai_max_context_messages()),
        ContextScope::Full => Some(state.ai_analysis_context_messages()),
    };
    let room_context = match (execution.room_id, context_limit) {
        (Some(room_id), Some(limit)) => Some(
            room_context_for_authorized_user_with_limit(state, execution.user_id, room_id, limit)
                .await
                .map_err(|status| anyhow::anyhow!("room context unavailable: {status}"))?,
        ),
        _ => None,
    };
    let history = completed_thread_history(state, execution).await?;
    let attachment_sources = room_context
        .as_ref()
        .map(|context| context.sources.clone())
        .unwrap_or_default();
    let mut context = GenerationContext {
        history,
        message_count: room_context
            .as_ref()
            .map_or(0, |context| context.context_message_count) as i64,
        toon_context: room_context
            .as_ref()
            .map(|context| context.toon_context.clone()),
        retrieved_message_count: 0,
        sources: attachment_sources,
    };
    if !context.sources.is_empty() {
        progress
            .publish_step(
                state,
                execution,
                RunStep::new(
                    RunStage::PreparingContext,
                    "context_attachments",
                    "读取对话附件",
                    format!("发现 {} 个可引用的文件或图片", context.sources.len()),
                ),
                "",
            )
            .await?;
    }
    let Some(room_id) = execution.room_id else {
        progress
            .publish_step(
                state,
                execution,
                RunStep::new(
                    RunStage::PreparingContext,
                    "rag_skipped",
                    "跳过语义检索",
                    "本次对话未关联聊天室",
                ),
                "",
            )
            .await?;
        return Ok(context);
    };
    if !plan.semantic_search {
        progress
            .publish_step(
                state,
                execution,
                RunStep::new(
                    RunStage::PreparingContext,
                    "rag_skipped",
                    "跳过语义检索",
                    "执行计划已装载全房间历史，无需补充相似消息",
                ),
                "",
            )
            .await?;
        return Ok(context);
    }
    let excluded_message_ids = room_context
        .map(|context| context.message_ids)
        .unwrap_or_default();
    let Some(index) = state.message_index().cloned() else {
        progress
            .publish_step(
                state,
                execution,
                RunStep::new(
                    RunStage::PreparingContext,
                    "rag_skipped",
                    "跳过语义检索",
                    "向量检索未启用",
                ),
                "",
            )
            .await?;
        return Ok(context);
    };
    let queries = if plan.research_questions.is_empty() {
        vec![execution.question.clone()]
    } else {
        plan.research_questions.clone()
    };
    progress.set_context(context.message_count, 0, &context.sources);
    progress
        .publish_step(
            state,
            execution,
            RunStep::new(
                RunStage::RetrievingContext,
                "rag_parallel",
                if queries.len() > 1 {
                    "并行执行研究 Agent"
                } else {
                    "执行语义检索 Agent"
                },
                format!(
                    "{} 个检索任务；Embedding：{}；Rerank：{}",
                    queries.len(),
                    index.embedding_model(),
                    index.rerank_model().unwrap_or("未配置")
                ),
            ),
            "",
        )
        .await?;
    let mut results: FuturesUnordered<_> = queries
        .into_iter()
        .enumerate()
        .map(|(agent_index, query)| {
            let state = state.clone();
            let index = index.clone();
            let excluded = excluded_message_ids.clone();
            async move {
                let started = tokio::time::Instant::now();
                let result = tokio::time::timeout(
                    SEMANTIC_SEARCH_TIMEOUT,
                    retrieve_room_context(
                        state,
                        index,
                        execution.user_id,
                        room_id,
                        &query,
                        excluded,
                        agent_index * 100,
                    ),
                )
                .await;
                (agent_index, query, started.elapsed(), result)
            }
        })
        .collect();
    while let Some((agent_index, query, elapsed, result)) = results.next().await {
        let (count, rag_context, outcome) = match result {
            Ok(Ok(rag_context)) if rag_context.message_count > 0 => {
                (rag_context.message_count, Some(rag_context), "完成")
            }
            Ok(Ok(_)) => (0, None, "未发现达到相关度阈值的证据"),
            Ok(Err(error)) => {
                tracing::warn!(%room_id, agent = agent_index + 1, "research retrieval failed: {error:#}");
                (0, None, "检索失败，已降级")
            }
            Err(_) => {
                tracing::warn!(%room_id, agent = agent_index + 1, "research retrieval timed out");
                (0, None, "检索超时，已降级")
            }
        };
        progress
            .publish_step(
                state,
                execution,
                RunStep::new(
                    RunStage::RetrievingContext,
                    format!("rag_agent_{}", agent_index + 1),
                    "研究 Agent 完成",
                    format!(
                        "Agent {}：{}；{}；选中 {} 条；{} ms",
                        agent_index + 1,
                        truncate_detail(&query, 80),
                        outcome,
                        count,
                        elapsed.as_millis()
                    ),
                ),
                "",
            )
            .await?;
        if let Some(rag_context) = rag_context {
            context.message_count += rag_context.message_count as i64;
            context.retrieved_message_count += rag_context.message_count as i64;
            context.sources.extend(rag_context.sources);
            append_context(&mut context.toon_context, &rag_context.toon_context);
        }
    }
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
                RunStage::RetrievingContext,
                "rag_selected",
                "汇总研究证据",
                format!(
                    "并行检索完成，共注入 {} 条证据",
                    context.retrieved_message_count
                ),
            ),
            "",
        )
        .await?;
    Ok(context)
}

async fn completed_thread_history(
    state: &SharedState,
    execution: &AiRunExecution,
) -> anyhow::Result<Vec<AiConversationTurn>> {
    Ok(state
        .ai_thread_messages(execution.user_id, execution.thread_id, MEMORY_TURNS)
        .await?
        .ok_or_else(|| anyhow::anyhow!("AI thread no longer exists"))?
        .into_iter()
        .filter(|message| !execution.excludes(message) && message.status == "completed")
        .map(|message| AiConversationTurn {
            role: message.role,
            content: message.content,
        })
        .collect())
}

fn truncate_detail(value: &str, limit: usize) -> String {
    let mut chars = value.chars();
    let truncated: String = chars.by_ref().take(limit).collect();
    if chars.next().is_some() {
        format!("{truncated}…")
    } else {
        truncated
    }
}

fn append_context(context: &mut Option<String>, evidence: &str) {
    let prompt_context = context.get_or_insert_with(String::new);
    if !prompt_context.is_empty() {
        prompt_context.push_str("\n\n");
    }
    prompt_context.push_str(evidence);
}
