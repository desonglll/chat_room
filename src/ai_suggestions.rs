//! Governed synchronous and streaming Room reply suggestions.

use std::{convert::Infallible, time::Duration};

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::sse::{Event, KeepAlive, Sse},
    Json,
};
use futures_util::{Stream, StreamExt};
use uuid::Uuid;

use crate::{
    ai::{
        model_options::ResolvedAiModel, AiAssistant, AiContextMessage, AiStreamItem, AiSuggestions,
    },
    ai_governance::{estimate_tokens, AiAdmissionRequest, GovernedAiStream},
    ai_handlers::require_room_password,
    state::SharedState,
    user_handlers::bearer_token,
};

#[utoipa::path(
    post,
    path = "/api/rooms/{id}/ai/suggest",
    params(("id" = Uuid, Path, description = "Room ID")),
    responses(
        (status = 200, description = "Summary and suggested replies", body = AiSuggestions),
        (status = 401, description = "Missing or expired session"),
        (status = 403, description = "Room AI policy or posting permission denied"),
        (status = 404, description = "Room not found"),
        (status = 429, description = "Cooldown, concurrency, or usage limit reached"),
        (status = 503, description = "AI model is disabled, blocked, or unavailable")
    )
)]
pub async fn suggest(
    State(state): State<SharedState>,
    Path(room_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<AiSuggestions>, StatusCode> {
    let (assistant, model, user_id, room_name, context) =
        prepare_suggestion(&state, room_id, &headers).await?;
    let input_tokens = suggestion_input_tokens(&room_name, &context);
    let admission = state
        .admit_ai(AiAdmissionRequest {
            user_id,
            room_id: Some(room_id),
            feature: "suggestion",
            model: &model,
            reserved_tokens: input_tokens,
        })
        .await
        .map_err(|error| error.status())?;
    match assistant.suggest(&room_name, &context).await {
        Ok(suggestions) => {
            let output_tokens = estimate_tokens(
                std::iter::once(suggestions.summary.as_str())
                    .chain(suggestions.suggestions.iter().map(String::as_str)),
            );
            if let Err(error) = state
                .finish_ai_admission(admission.id, "completed", Some(input_tokens), output_tokens)
                .await
            {
                tracing::error!(%room_id, "record AI suggestion usage failed: {error}");
            }
            Ok(Json(suggestions))
        }
        Err(error) => {
            let _ = state
                .finish_ai_admission(admission.id, "failed", Some(input_tokens), 0)
                .await;
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
        (status = 403, description = "Room AI policy or posting permission denied"),
        (status = 429, description = "Cooldown, concurrency, or usage limit reached"),
        (status = 503, description = "AI model is disabled, blocked, or unavailable")
    )
)]
pub async fn suggest_events(
    State(state): State<SharedState>,
    Path(room_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, StatusCode> {
    let (assistant, model, user_id, room_name, context) =
        prepare_suggestion(&state, room_id, &headers).await?;
    let input_tokens = suggestion_input_tokens(&room_name, &context);
    let admission = state
        .admit_ai(AiAdmissionRequest {
            user_id,
            room_id: Some(room_id),
            feature: "suggestion",
            model: &model,
            reserved_tokens: input_tokens,
        })
        .await
        .map_err(|error| error.status())?;
    let stream = assistant.suggestion_stream(&room_name, &context).await;
    let stream = match stream {
        Ok(stream) => stream,
        Err(error) => {
            let _ = state
                .finish_ai_admission(admission.id, "failed", Some(input_tokens), 0)
                .await;
            tracing::error!(%room_id, "connect AI suggestion stream failed: {error:#}");
            return Err(StatusCode::SERVICE_UNAVAILABLE);
        }
    };
    let stream = GovernedAiStream::new(stream, state.clone(), admission.id, input_tokens);
    let events = stream.filter_map(move |item| async move {
        let chunk = match item {
            Ok(AiStreamItem::Content(chunk)) => chunk,
            Ok(AiStreamItem::Reasoning) => return None,
            Err(error) => {
                tracing::error!(%room_id, "AI suggestion stream failed: {error:#}");
                "\n{\"type\":\"error\",\"content\":\"AI 助手当前不可用\"}\n".into()
            }
        };
        Some(Ok(Event::default()
            .event("chunk")
            .json_data(chunk)
            .unwrap_or_else(|_| {
                Event::default().event("error").data("serialization failed")
            })))
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
) -> Result<
    (
        AiAssistant,
        ResolvedAiModel,
        Uuid,
        String,
        Vec<AiContextMessage>,
    ),
    StatusCode,
> {
    let user = state
        .session_user(bearer_token(headers)?)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::UNAUTHORIZED)?;
    let room = state.room(room_id).await.ok_or(StatusCode::NOT_FOUND)?;
    if !state
        .has_room_permission(room_id, user.id, "message.send")
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    {
        return Err(StatusCode::FORBIDDEN);
    }
    require_room_password(&room, headers)?;
    let assistant = state
        .ai_assistant()
        .cloned()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    let model = state
        .resolve_ai_model(None, false)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    if !state
        .check_action_cooldown(room_id, user.id, user.id, state.ai_suggest_cooldown())
        .await
    {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }
    let history = state
        .message_history(
            room_id,
            state.ai_max_context_messages() as i64,
            None,
            Some(user.id),
        )
        .await
        .map_err(|error| {
            tracing::error!("load AI context history failed: {error}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    let context = history
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
    Ok((assistant, model, user.id, room.name, context))
}

fn suggestion_input_tokens(room_name: &str, context: &[AiContextMessage]) -> i64 {
    estimate_tokens(
        std::iter::once(room_name).chain(context.iter().flat_map(|message| {
            [
                message.sender.as_str(),
                message.content.as_str(),
                message.attachment.as_str(),
            ]
        })),
    )
}
