//! AI endpoints for suggestions and conversation-grounded questions.

use std::convert::Infallible;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::sse::{Event, KeepAlive, Sse},
    Json,
};
use futures_util::{stream, Stream, StreamExt};
use serde::Serialize;
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::ai::{
    bounded_conversation_context_to_toon, AiContextMessage, AiConversationRequest,
    AiConversationResponse, AiConversationTurn, AiSuggestions,
};
use crate::handlers::authorize_room;
use crate::models::Room;
use crate::state::SharedState;
use crate::user_handlers::bearer_token;

/// Summarize the recent conversation and suggest a few replies the caller might
/// send next.
#[utoipa::path(
    post,
    path = "/api/rooms/{id}/ai/suggest",
    params(("id" = Uuid, Path, description = "Room ID")),
    responses(
        (status = 200, description = "Summary and suggested replies", body = AiSuggestions),
        (status = 401, description = "Missing or expired session"),
        (status = 403, description = "Not permitted to post in this room"),
        (status = 404, description = "Room not found"),
        (status = 429, description = "Too many requests, try again shortly"),
        (status = 503, description = "AI assistant is disabled or unavailable")
    )
)]
pub async fn suggest(
    State(state): State<SharedState>,
    Path(room_id): Path<Uuid>,
    headers: axum::http::HeaderMap,
) -> Result<Json<AiSuggestions>, StatusCode> {
    let token = bearer_token(&headers)?;
    let user = state
        .session_user(token)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::UNAUTHORIZED)?;

    let room = state.room(room_id).await.ok_or(StatusCode::NOT_FOUND)?;

    let can_send = state
        .has_room_permission(room_id, user.id, "message.send")
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if !can_send {
        return Err(StatusCode::FORBIDDEN);
    }
    require_room_password(&room, &headers)?;
    let Some(assistant) = state.ai_assistant() else {
        return Err(StatusCode::SERVICE_UNAVAILABLE);
    };

    if !state
        .check_action_cooldown(room_id, user.id, user.id, state.ai_suggest_cooldown())
        .await
    {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }

    let limit = state.ai_max_context_messages();
    let history = state
        .message_history(room_id, limit as i64, None, Some(user.id))
        .await
        .map_err(|error| {
            tracing::error!("load AI context history failed: {}", error);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let context: Vec<AiContextMessage> = history
        .into_iter()
        .filter(|message| message.recalled_at.is_none())
        .map(|message| AiContextMessage {
            sent_at: message.created_at.to_rfc3339(),
            sender: message.sender,
            content: message.content,
            attachment: message
                .attachment
                .map(|attachment| attachment.file_name)
                .unwrap_or_default(),
        })
        .collect();

    match assistant.suggest(&room.name, &context).await {
        Ok(suggestions) => Ok(Json(suggestions)),
        Err(error) => {
            tracing::error!("AI suggest request failed: {}", error);
            Err(StatusCode::SERVICE_UNAVAILABLE)
        }
    }
}

const MAX_QUESTION_CHARS: usize = 4_000;
const MAX_TURN_CHARS: usize = 4_000;
const MAX_HISTORY_TURNS: usize = 12;
const MAX_CONTEXT_MESSAGE_CHARS: usize = 1_500;
const MAX_CONTEXT_TOON_BYTES: usize = 256 * 1024;

#[utoipa::path(
    post,
    path = "/api/ai/conversations/{id}/query",
    params(("id" = Uuid, Path, description = "Conversation room ID")),
    request_body = AiConversationRequest,
    responses(
        (status = 200, description = "Answer grounded in the selected conversation", body = AiConversationResponse),
        (status = 400, description = "Invalid question or history"),
        (status = 401, description = "Missing or expired session"),
        (status = 403, description = "Not an active conversation member"),
        (status = 404, description = "Room not found"),
        (status = 429, description = "Too many requests, try again shortly"),
        (status = 503, description = "AI assistant is disabled, unconfigured, or unavailable")
    )
)]
pub async fn analyze_conversation(
    State(state): State<SharedState>,
    Path(room_id): Path<Uuid>,
    headers: axum::http::HeaderMap,
    Json(payload): Json<AiConversationRequest>,
) -> Result<Json<AiConversationResponse>, StatusCode> {
    let prepared = prepare_conversation_query(&state, room_id, &headers, payload).await?;
    let answer = prepared
        .assistant
        .answer(
            &prepared.toon_context,
            &prepared.history,
            &prepared.question,
        )
        .await
        .map_err(|error| {
            tracing::error!(%room_id, "AI conversation query failed: {error}");
            StatusCode::SERVICE_UNAVAILABLE
        })?;
    Ok(Json(AiConversationResponse {
        room_id,
        answer,
        context_message_count: prepared.context_message_count,
        context_format: "toon-v3-compatible".into(),
    }))
}

#[utoipa::path(
    post,
    path = "/api/ai/conversations/{id}/query/stream",
    params(("id" = Uuid, Path, description = "Conversation room ID")),
    request_body = AiConversationRequest,
    responses(
        (status = 200, description = "SSE stream with meta, delta, done, or error events", content_type = "text/event-stream"),
        (status = 400, description = "Invalid question or history"),
        (status = 401, description = "Missing or expired session"),
        (status = 403, description = "Not an active conversation member"),
        (status = 404, description = "Room not found"),
        (status = 429, description = "Too many requests, try again shortly"),
        (status = 503, description = "AI assistant is disabled, unconfigured, or unavailable")
    )
)]
pub async fn analyze_conversation_stream(
    State(state): State<SharedState>,
    Path(room_id): Path<Uuid>,
    headers: axum::http::HeaderMap,
    Json(payload): Json<AiConversationRequest>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, StatusCode> {
    let prepared = prepare_conversation_query(&state, room_id, &headers, payload).await?;
    let mut provider_stream = prepared
        .assistant
        .answer_stream(
            &prepared.toon_context,
            &prepared.history,
            &prepared.question,
        )
        .await
        .map_err(|error| {
            tracing::error!(%room_id, "start AI conversation stream failed: {error}");
            StatusCode::SERVICE_UNAVAILABLE
        })?;
    let (sender, receiver) = mpsc::channel(16);
    tokio::spawn(async move {
        let meta = ConversationStreamMeta {
            room_id,
            context_message_count: prepared.context_message_count,
            context_format: "toon-v3-compatible",
        };
        if sender.send(json_event("meta", meta)).await.is_err() {
            return;
        }
        while let Some(chunk) = provider_stream.next().await {
            match chunk {
                Ok(content) => {
                    if sender
                        .send(json_event("delta", ConversationStreamDelta { content }))
                        .await
                        .is_err()
                    {
                        return;
                    }
                }
                Err(error) => {
                    tracing::error!(%room_id, "AI conversation stream failed: {error}");
                    let _ = sender
                        .send(json_event(
                            "error",
                            ConversationStreamError {
                                message: "AI 助手当前不可用",
                            },
                        ))
                        .await;
                    return;
                }
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

struct PreparedConversationQuery {
    assistant: crate::ai::AiAssistant,
    toon_context: String,
    history: Vec<AiConversationTurn>,
    question: String,
    context_message_count: usize,
}

async fn prepare_conversation_query(
    state: &SharedState,
    room_id: Uuid,
    headers: &axum::http::HeaderMap,
    payload: AiConversationRequest,
) -> Result<PreparedConversationQuery, StatusCode> {
    let token = bearer_token(headers)?;
    let user = state
        .session_user(token)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::UNAUTHORIZED)?;
    let room = state.room(room_id).await.ok_or(StatusCode::NOT_FOUND)?;
    let conversation = state
        .conversation_summary(user.id, room_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::FORBIDDEN)?;
    require_room_password(&room, headers)?;
    validate_query(&payload)?;
    let Some(assistant) = state.ai_assistant().cloned() else {
        return Err(StatusCode::SERVICE_UNAVAILABLE);
    };
    if !state
        .check_action_cooldown(room_id, user.id, user.id, state.ai_suggest_cooldown())
        .await
    {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }
    let history = state
        .message_history(
            room.id,
            state.ai_analysis_context_messages() as i64,
            None,
            Some(user.id),
        )
        .await
        .map_err(|error| {
            tracing::error!(%room_id, "load AI analysis context failed: {error}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    let mut context: Vec<AiContextMessage> = history
        .into_iter()
        .filter(|message| message.recalled_at.is_none())
        .map(|message| AiContextMessage {
            sent_at: message.created_at.to_rfc3339(),
            sender: message.sender,
            content: truncate_chars(&message.content, MAX_CONTEXT_MESSAGE_CHARS),
            attachment: message
                .attachment
                .map(|attachment| attachment.file_name)
                .unwrap_or_default(),
        })
        .collect();
    let toon_context = bounded_conversation_context_to_toon(
        &conversation.title,
        &mut context,
        MAX_CONTEXT_TOON_BYTES,
    )
    .map_err(|error| {
        tracing::error!(%room_id, "encode AI analysis context failed: {error}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    Ok(PreparedConversationQuery {
        assistant,
        toon_context,
        history: payload.history,
        question: payload.question.trim().to_owned(),
        context_message_count: context.len(),
    })
}

#[derive(Serialize)]
struct ConversationStreamMeta {
    room_id: Uuid,
    context_message_count: usize,
    context_format: &'static str,
}

#[derive(Serialize)]
struct ConversationStreamDelta {
    content: String,
}

#[derive(Serialize)]
struct ConversationStreamError {
    message: &'static str,
}

#[derive(Serialize)]
struct EmptyEvent {}

fn json_event(name: &'static str, payload: impl Serialize) -> Event {
    Event::default()
        .event(name)
        .json_data(payload)
        .expect("AI stream events must be serializable")
}

fn validate_query(payload: &AiConversationRequest) -> Result<(), StatusCode> {
    let question_len = payload.question.trim().chars().count();
    let valid_history =
        payload.history.len() <= MAX_HISTORY_TURNS && payload.history.iter().all(valid_turn);
    if question_len == 0 || question_len > MAX_QUESTION_CHARS || !valid_history {
        return Err(StatusCode::BAD_REQUEST);
    }
    Ok(())
}

fn require_room_password(room: &Room, headers: &axum::http::HeaderMap) -> Result<(), StatusCode> {
    if !room.has_password {
        return Ok(());
    }
    let supplied = headers
        .get("x-room-password")
        .and_then(|value| value.to_str().ok());
    authorize_room(room, supplied)
        .then_some(())
        .ok_or(StatusCode::UNAUTHORIZED)
}

fn valid_turn(turn: &AiConversationTurn) -> bool {
    matches!(turn.role.as_str(), "user" | "assistant")
        && !turn.content.trim().is_empty()
        && turn.content.chars().count() <= MAX_TURN_CHARS
}

fn truncate_chars(value: &str, limit: usize) -> String {
    if value.chars().count() <= limit {
        return value.to_owned();
    }
    let mut truncated: String = value.chars().take(limit.saturating_sub(1)).collect();
    truncated.push('…');
    truncated
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conversation_queries_are_bounded_and_role_checked() {
        let valid = AiConversationRequest {
            question: "总结一下".into(),
            history: vec![AiConversationTurn {
                role: "assistant".into(),
                content: "上一轮回答".into(),
            }],
        };
        assert!(validate_query(&valid).is_ok());
        let invalid = AiConversationRequest {
            question: " ".into(),
            history: vec![],
        };
        assert_eq!(validate_query(&invalid), Err(StatusCode::BAD_REQUEST));
    }
}
