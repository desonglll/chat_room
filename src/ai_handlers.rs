//! AI suggestions and authorized room-context preparation.

use std::{collections::HashSet, convert::Infallible, time::Duration};

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::sse::{Event, KeepAlive, Sse},
    Json,
};
use futures_util::{Stream, StreamExt};
use uuid::Uuid;

use crate::ai::{
    bounded_conversation_context_to_toon, AiAssistant, AiContextMessage, AiStreamItem,
    AiSuggestions,
};
use crate::ai_threads::{AiCitationAttachment, AiCitationSource};
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
    let (assistant, room_name, context) = prepare_suggestion(&state, room_id, &headers).await?;
    match assistant.suggest(&room_name, &context).await {
        Ok(suggestions) => Ok(Json(suggestions)),
        Err(error) => {
            tracing::error!(%room_id, "AI suggest request failed: {error:#}");
            Err(StatusCode::SERVICE_UNAVAILABLE)
        }
    }
}

#[utoipa::path(
    post,
    path = "/api/rooms/{id}/ai/suggest/events",
    params(("id" = Uuid, Path, description = "Room ID")),
    responses(
        (status = 200, description = "Streaming NDJSON suggestion chunks over server-sent events"),
        (status = 401, description = "Missing or expired session"),
        (status = 403, description = "Not permitted to post in this room"),
        (status = 429, description = "Too many requests, try again shortly"),
        (status = 503, description = "AI assistant is disabled or unavailable")
    )
)]
pub async fn suggest_events(
    State(state): State<SharedState>,
    Path(room_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, StatusCode> {
    let (assistant, room_name, context) = prepare_suggestion(&state, room_id, &headers).await?;
    let stream = assistant
        .suggestion_stream(&room_name, &context)
        .await
        .map_err(|error| {
            tracing::error!(%room_id, "connect AI suggestion stream failed: {error:#}");
            StatusCode::SERVICE_UNAVAILABLE
        })?;
    let events = stream.filter_map(move |item| async move {
        let chunk = match item {
            Ok(AiStreamItem::Content(chunk)) => chunk,
            Ok(AiStreamItem::Reasoning) => return None,
            Err(error) => {
                tracing::error!(%room_id, "AI suggestion stream failed: {error:#}");
                "\n{\"type\":\"error\",\"content\":\"AI 助手当前不可用\"}\n".into()
            }
        };
        let event = Event::default()
            .event("chunk")
            .json_data(chunk)
            .unwrap_or_else(|_| Event::default().event("error").data("serialization failed"));
        Some(Ok(event))
    });
    Ok(Sse::new(events).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive"),
    ))
}

async fn prepare_suggestion(
    state: &SharedState,
    room_id: Uuid,
    headers: &HeaderMap,
) -> Result<(AiAssistant, String, Vec<AiContextMessage>), StatusCode> {
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
    let Some(assistant) = state.ai_assistant().cloned() else {
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
            message_id: message.id.to_string(),
            sent_at: message.created_at.to_rfc3339(),
            sender: message.sender,
            content: message.content,
            source: String::new(),
            attachment: message
                .attachment
                .map(|attachment| attachment.file_name)
                .unwrap_or_default(),
        })
        .collect();

    Ok((assistant, room.name, context))
}

const MAX_CONTEXT_MESSAGE_CHARS: usize = 1_500;
const MAX_CONTEXT_TOON_BYTES: usize = 256 * 1024;

pub(crate) struct PreparedRoomContext {
    pub toon_context: String,
    pub context_message_count: usize,
    pub message_ids: HashSet<Uuid>,
    pub sources: Vec<AiCitationSource>,
}

pub(crate) async fn room_context_for_user(
    state: &SharedState,
    user_id: Uuid,
    room_id: Uuid,
    headers: &axum::http::HeaderMap,
) -> Result<PreparedRoomContext, StatusCode> {
    let room = state.room(room_id).await.ok_or(StatusCode::NOT_FOUND)?;
    state
        .conversation_summary(user_id, room_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::FORBIDDEN)?;
    require_room_password(&room, headers)?;
    room_context_for_authorized_user(state, user_id, room_id).await
}

pub(crate) async fn room_context_for_authorized_user(
    state: &SharedState,
    user_id: Uuid,
    room_id: Uuid,
) -> Result<PreparedRoomContext, StatusCode> {
    room_context_for_authorized_user_with_limit(
        state,
        user_id,
        room_id,
        state.ai_analysis_context_messages(),
    )
    .await
}

pub(crate) async fn room_context_for_authorized_user_with_limit(
    state: &SharedState,
    user_id: Uuid,
    room_id: Uuid,
    limit: usize,
) -> Result<PreparedRoomContext, StatusCode> {
    let room = state.room(room_id).await.ok_or(StatusCode::NOT_FOUND)?;
    let conversation = state
        .conversation_summary(user_id, room_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::FORBIDDEN)?;
    let history = state
        .message_history(room.id, limit as i64, None, Some(user_id))
        .await
        .map_err(|error| {
            tracing::error!(%room_id, "load AI analysis context failed: {error}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    let mut sources = Vec::new();
    let mut context = Vec::new();
    for message in history
        .into_iter()
        .filter(|message| message.recalled_at.is_none())
    {
        let source = message
            .attachment
            .as_ref()
            .map_or_else(String::new, |attachment| {
                let label = format!("A{}", sources.len() + 1);
                sources.push(AiCitationSource {
                    label: label.clone(),
                    room_id,
                    message_id: message.id,
                    sender: message.sender.clone(),
                    sent_at: message.created_at,
                    excerpt: if message.content.trim().is_empty() {
                        attachment.file_name.clone()
                    } else {
                        truncate_chars(&message.content, 280)
                    },
                    score: None,
                    score_kind: "attachment".into(),
                    attachment: Some(AiCitationAttachment {
                        id: attachment.id,
                        file_name: attachment.file_name.clone(),
                        mime_type: attachment.mime_type.clone(),
                        size_bytes: attachment.size_bytes,
                        download_url: attachment.download_url.clone(),
                        is_sensitive: attachment.is_sensitive,
                    }),
                });
                label
            });
        let attachment = message.attachment.map_or_else(String::new, |attachment| {
            format!(
                "{} ({}, {} bytes)",
                attachment.file_name, attachment.mime_type, attachment.size_bytes
            )
        });
        context.push(AiContextMessage {
            message_id: message.id.to_string(),
            sent_at: message.created_at.to_rfc3339(),
            sender: message.sender,
            content: truncate_chars(&message.content, MAX_CONTEXT_MESSAGE_CHARS),
            source,
            attachment,
        });
    }
    let toon_context = bounded_conversation_context_to_toon(
        &conversation.title,
        &mut context,
        MAX_CONTEXT_TOON_BYTES,
    )
    .map_err(|error| {
        tracing::error!(%room_id, "encode AI analysis context failed: {error}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let active_sources: HashSet<&str> = context
        .iter()
        .map(|message| message.source.as_str())
        .filter(|source| !source.is_empty())
        .collect();
    sources.retain(|source| active_sources.contains(source.label.as_str()));
    let message_ids = context
        .iter()
        .filter_map(|message| Uuid::parse_str(&message.message_id).ok())
        .collect();
    Ok(PreparedRoomContext {
        toon_context,
        context_message_count: context.len(),
        message_ids,
        sources,
    })
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

fn truncate_chars(value: &str, limit: usize) -> String {
    if value.chars().count() <= limit {
        return value.to_owned();
    }
    let mut truncated: String = value.chars().take(limit.saturating_sub(1)).collect();
    truncated.push('…');
    truncated
}
