use std::time::Duration;

use crate::ai::AiConversationTurn;
use crate::ai_handlers::room_context_for_authorized_user;
use crate::knowledge::retrieve_room_context;
use crate::state::SharedState;

use super::models::AiCitationSource;
use super::run_store::AiRunExecution;

const MEMORY_TURNS: i64 = 26;
const SEMANTIC_SEARCH_TIMEOUT: Duration = Duration::from_millis(1_500);

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
        sources: Vec::new(),
    };
    let (Some(room_id), Some(index)) = (execution.room_id, state.message_index().cloned()) else {
        return Ok(context);
    };
    let excluded_message_ids = room_context
        .map(|context| context.message_ids)
        .unwrap_or_default();
    match tokio::time::timeout(
        SEMANTIC_SEARCH_TIMEOUT,
        retrieve_room_context(
            state.clone(),
            index,
            execution.user_id,
            room_id,
            &execution.question,
            excluded_message_ids,
        ),
    )
    .await
    {
        Ok(Ok(rag_context)) if rag_context.message_count > 0 => {
            context.message_count += rag_context.message_count as i64;
            context.retrieved_message_count = rag_context.message_count as i64;
            context.sources = rag_context.sources;
            let prompt_context = context.toon_context.get_or_insert_with(String::new);
            prompt_context.push_str("\n\n");
            prompt_context.push_str(&rag_context.toon_context);
        }
        Ok(Ok(_)) => {}
        Ok(Err(error)) => {
            tracing::warn!(room_id = %room_id, "semantic message retrieval failed: {error:#}");
        }
        Err(_) => tracing::warn!(room_id = %room_id, "semantic message retrieval timed out"),
    }
    Ok(context)
}
