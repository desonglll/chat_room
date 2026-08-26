use std::time::Duration;

use crate::ai::AiConversationTurn;
use crate::ai_handlers::room_context_for_authorized_user;
use crate::knowledge::retrieve_room_context;
use crate::knowledge_graph::retrieve_graph_context;
use crate::state::SharedState;

use super::run_store::AiRunExecution;

const MEMORY_TURNS: i64 = 26;
const SEMANTIC_SEARCH_TIMEOUT: Duration = Duration::from_millis(1_500);

pub(super) struct GenerationContext {
    pub history: Vec<AiConversationTurn>,
    pub toon_context: Option<String>,
    pub message_count: i64,
    pub retrieved_message_count: i64,
}

pub(super) async fn prepare_generation_context(
    state: &SharedState,
    execution: &AiRunExecution,
) -> anyhow::Result<GenerationContext> {
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
        .ok_or_else(|| anyhow::anyhow!("AI thread no longer exists"))?
        .into_iter()
        .filter(|message| !execution.excludes(message) && message.status == "completed")
        .map(|message| AiConversationTurn {
            role: message.role,
            content: message.content,
        })
        .collect();
    let mut context = GenerationContext {
        history,
        message_count: room_context
            .as_ref()
            .map_or(0, |context| context.context_message_count) as i64,
        toon_context: room_context
            .as_ref()
            .map(|context| context.toon_context.clone()),
        retrieved_message_count: 0,
    };
    let Some(room_id) = execution.room_id else {
        return Ok(context);
    };
    let excluded_message_ids = room_context
        .map(|context| context.message_ids)
        .unwrap_or_default();
    let vector_excluded_ids = excluded_message_ids.clone();
    let vector_retrieval = async {
        let index = state.message_index().cloned()?;
        Some(
            tokio::time::timeout(
                SEMANTIC_SEARCH_TIMEOUT,
                retrieve_room_context(
                    state.clone(),
                    index,
                    execution.user_id,
                    room_id,
                    &execution.question,
                    vector_excluded_ids,
                ),
            )
            .await,
        )
    };
    let graph_retrieval = async {
        let graph = state.knowledge_graph().cloned()?;
        Some(
            tokio::time::timeout(
                graph.search_timeout,
                retrieve_graph_context(
                    state.clone(),
                    graph,
                    execution.user_id,
                    room_id,
                    &execution.question,
                    excluded_message_ids,
                ),
            )
            .await,
        )
    };
    let (vector_result, graph_result) = tokio::join!(vector_retrieval, graph_retrieval);
    match vector_result {
        None => {}
        Some(Ok(Ok(rag_context))) if rag_context.message_count > 0 => {
            context.message_count += rag_context.message_count as i64;
            context.retrieved_message_count += rag_context.message_count as i64;
            append_context(&mut context.toon_context, &rag_context.toon_context);
        }
        Some(Ok(Ok(_))) => {}
        Some(Ok(Err(error))) => {
            tracing::warn!(room_id = %room_id, "semantic message retrieval failed: {error:#}");
        }
        Some(Err(_)) => tracing::warn!(room_id = %room_id, "semantic message retrieval timed out"),
    }
    match graph_result {
        None => {}
        Some(Ok(Ok(graph_context))) if graph_context.fact_count > 0 => {
            context.message_count += graph_context.fact_count as i64;
            context.retrieved_message_count += graph_context.fact_count as i64;
            append_context(&mut context.toon_context, &graph_context.toon_context);
        }
        Some(Ok(Ok(_))) => {}
        Some(Ok(Err(error))) => {
            tracing::warn!(room_id = %room_id, "knowledge graph retrieval failed: {error:#}");
        }
        Some(Err(_)) => tracing::warn!(room_id = %room_id, "knowledge graph retrieval timed out"),
    }
    Ok(context)
}

fn append_context(context: &mut Option<String>, evidence: &str) {
    let prompt_context = context.get_or_insert_with(String::new);
    if !prompt_context.is_empty() {
        prompt_context.push_str("\n\n");
    }
    prompt_context.push_str(evidence);
}
