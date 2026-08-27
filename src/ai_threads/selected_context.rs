use std::collections::HashSet;

use axum::http::StatusCode;
use uuid::Uuid;

use crate::{
    models::StoredMessage,
    state::{with_pool, AppState, SharedState},
};

use super::{
    catch_up_context::build_message_context, context::GenerationContext, run_store::AiRunExecution,
};

const MAX_SELECTED_MESSAGES: usize = 50;

pub(super) async fn validate_selected_messages(
    state: &SharedState,
    user_id: Uuid,
    room_id: Option<Uuid>,
    message_ids: &[Uuid],
) -> Result<(), StatusCode> {
    if message_ids.is_empty() {
        return Ok(());
    }
    if message_ids.len() > MAX_SELECTED_MESSAGES
        || message_ids.iter().collect::<HashSet<_>>().len() != message_ids.len()
    {
        return Err(StatusCode::BAD_REQUEST);
    }
    let room_id = room_id.ok_or(StatusCode::BAD_REQUEST)?;
    for message_id in message_ids {
        state
            .message_by_id(*message_id, Some(user_id))
            .await
            .map_err(|error| {
                tracing::error!(%room_id, "validate selected AI message failed: {error}");
                StatusCode::INTERNAL_SERVER_ERROR
            })?
            .filter(|message| message.room_id == room_id && message.recalled_at.is_none())
            .ok_or(StatusCode::BAD_REQUEST)?;
    }
    Ok(())
}

impl AppState {
    pub(super) async fn ai_run_selected_message_ids(
        &self,
        run_id: Uuid,
    ) -> Result<Vec<Uuid>, sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query_scalar(
                "SELECT message_id FROM ai_run_selected_messages \
                 WHERE run_id = $1 ORDER BY ordinal",
            )
            .bind(run_id)
            .fetch_all(pool)
            .await
        })
    }
}

pub(super) async fn prepare_selected_context(
    state: &SharedState,
    execution: &AiRunExecution,
    message_ids: Vec<Uuid>,
) -> anyhow::Result<GenerationContext> {
    let room_id = execution
        .room_id
        .ok_or_else(|| anyhow::anyhow!("selected-message run has no room"))?;
    let conversation = state
        .conversation_summary(execution.user_id, room_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("active room membership is required"))?;
    let mut messages = Vec::<StoredMessage>::with_capacity(message_ids.len());
    for message_id in message_ids {
        let Some(message) = state
            .message_by_id(message_id, Some(execution.user_id))
            .await?
        else {
            continue;
        };
        if message.room_id == room_id && message.recalled_at.is_none() {
            messages.push(message);
        }
    }
    if messages.is_empty() {
        anyhow::bail!("selected message context is no longer available");
    }
    build_message_context(room_id, &conversation.title, messages)
}
