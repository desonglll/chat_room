use std::convert::Infallible;

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::sse::{Event, KeepAlive, Sse},
    Json,
};
use futures_util::{stream, Stream, StreamExt};
use serde::Serialize;
use tokio::sync::mpsc;
use uuid::Uuid;

use super::handlers::{current_user, internal_error, validate_room_access, DEFAULT_TITLE};
use super::models::QueryAiThreadRequest;
use crate::ai::{AiConversationTurn, AiStreamItem};
use crate::ai_handlers::room_context_for_user;
use crate::state::SharedState;

const MAX_QUESTION_CHARS: usize = 4_000;
const MEMORY_TURNS: i64 = 24;

#[utoipa::path(
    post,
    path = "/api/ai/threads/{id}/query/stream",
    params(("id" = Uuid, Path, description = "AI session id")),
    request_body = QueryAiThreadRequest,
    responses(
        (status = 200, description = "SSE stream with meta, status, delta, done, or error events", content_type = "text/event-stream"),
        (status = 400, description = "Invalid question"),
        (status = 404, description = "AI session not found"),
        (status = 503, description = "AI assistant is unavailable")
    )
)]
pub async fn query_thread_stream(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(thread_id): Path<Uuid>,
    Json(payload): Json<QueryAiThreadRequest>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, StatusCode> {
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
    let Some(assistant) = state.ai_assistant().cloned() else {
        return Err(StatusCode::SERVICE_UNAVAILABLE);
    };
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
    let room_context = match room_id {
        Some(room_id) => Some(room_context_for_user(&state, user.id, room_id, &headers).await?),
        None => None,
    };
    let history = state
        .ai_thread_messages(user.id, thread.id, MEMORY_TURNS)
        .await
        .map_err(internal_error)?
        .ok_or(StatusCode::NOT_FOUND)?;
    let history: Vec<AiConversationTurn> = history
        .into_iter()
        .map(|message| AiConversationTurn {
            role: message.role,
            content: message.content,
        })
        .collect();

    // debug for user question.
    // tracing::debug!("{}", question);
    state
        .append_ai_thread_message(user.id, thread.id, "user", question, room_id, None)
        .await
        .map_err(internal_error)?
        .ok_or(StatusCode::NOT_FOUND)?;
    if thread.title == DEFAULT_TITLE {
        let title = title_from_question(question);
        if let Some(updated) = state
            .update_ai_thread(
                user.id,
                thread.id,
                &title,
                thread.room_id,
                thread.thinking_enabled,
            )
            .await
            .map_err(internal_error)?
        {
            thread = updated;
        }
    }
    let context_message_count = room_context
        .as_ref()
        .map_or(0, |context| context.context_message_count);
    let mut provider_stream = assistant
        .answer_stream(
            room_context
                .as_ref()
                .map(|context| context.toon_context.as_str()),
            &history,
            question,
            thread.thinking_enabled,
        )
        .await
        .map_err(|error| {
            tracing::error!(%thread_id, "start AI thread stream failed: {error}");
            StatusCode::SERVICE_UNAVAILABLE
        })?;
    let (sender, receiver) = mpsc::channel(16);
    let persistence_state = state.clone();
    let title = thread.title.clone();
    tokio::spawn(async move {
        if sender
            .send(json_event(
                "meta",
                ThreadStreamMeta {
                    thread_id,
                    title,
                    room_id,
                    context_message_count,
                    context_format: room_id.map(|_| "toon-v3-compatible"),
                },
            ))
            .await
            .is_err()
        {
            return;
        }
        let mut answer = String::new();
        while let Some(item) = provider_stream.next().await {
            match item {
                Ok(AiStreamItem::Reasoning) => {
                    if sender
                        .send(json_event(
                            "status",
                            ThreadStreamStatus { phase: "reasoning" },
                        ))
                        .await
                        .is_err()
                    {
                        return;
                    }
                }
                Ok(AiStreamItem::Content(content)) => {
                    answer.push_str(&content);
                    if sender
                        .send(json_event("delta", ThreadStreamDelta { content }))
                        .await
                        .is_err()
                    {
                        return;
                    }
                }
                Err(error) => {
                    tracing::error!(%thread_id, "AI thread stream failed: {error}");
                    let _ = sender
                        .send(json_event(
                            "error",
                            ThreadStreamError {
                                message: "AI 助手当前不可用，请稍后重试",
                            },
                        ))
                        .await;
                    return;
                }
            }
        }
        if !answer.trim().is_empty() {
            if let Err(error) = persistence_state
                .append_ai_thread_message(
                    user.id,
                    thread_id,
                    "assistant",
                    answer.trim(),
                    room_id,
                    Some(context_message_count as i64),
                )
                .await
            {
                tracing::error!(%thread_id, "persist AI response failed: {error}");
            }
        }
        let _ = sender.send(json_event("done", EmptyEvent {})).await;
    });
    let output = stream::unfold(receiver, |mut receiver| async move {
        receiver
            .recv()
            .await
            .map(|event| (Ok::<_, Infallible>(event), receiver))
    });
    Ok(Sse::new(output).keep_alive(
        KeepAlive::new()
            .interval(std::time::Duration::from_secs(15))
            .text("keep-alive"),
    ))
}

fn title_from_question(question: &str) -> String {
    let mut title: String = question.chars().take(28).collect();
    if question.chars().count() > 28 {
        title.push('…');
    }
    title
}

fn json_event(name: &'static str, payload: impl Serialize) -> Event {
    Event::default()
        .event(name)
        .json_data(payload)
        .expect("AI thread stream events must be serializable")
}

#[derive(Serialize)]
struct ThreadStreamMeta {
    thread_id: Uuid,
    title: String,
    room_id: Option<Uuid>,
    context_message_count: usize,
    context_format: Option<&'static str>,
}

#[derive(Serialize)]
struct ThreadStreamStatus {
    phase: &'static str,
}

#[derive(Serialize)]
struct ThreadStreamDelta {
    content: String,
}

#[derive(Serialize)]
struct ThreadStreamError {
    message: &'static str,
}

#[derive(Serialize)]
struct EmptyEvent {}
