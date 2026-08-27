use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use uuid::Uuid;

use super::{
    create_run::{CatchUpRunSource, CreateRunOutcome, NewCatchUpRun},
    handlers::{current_user, internal_error, DEFAULT_TITLE},
    models::CreateCatchUpRunRequest,
    runs::spawn_run,
};
use crate::{
    ai_governance::AiAdmissionRequest, ai_handlers::require_room_password, state::SharedState,
};

#[utoipa::path(
    post,
    path = "/api/ai/threads/{id}/catch-up",
    params(("id" = Uuid, Path, description = "Personal AI session id")),
    request_body = CreateCatchUpRunRequest,
    responses(
        (status = 202, description = "Durable unread-summary run accepted", body = super::models::AiRun),
        (status = 204, description = "No unread messages; no model was invoked"),
        (status = 401, description = "Missing session or private-room password"),
        (status = 403, description = "Active room membership is required"),
        (status = 404, description = "AI session or room not found"),
        (status = 409, description = "The session already has an active run"),
        (status = 429, description = "Too many AI requests"),
        (status = 503, description = "AI assistant is unavailable")
    )
)]
pub async fn create_catch_up(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(thread_id): Path<Uuid>,
    Json(payload): Json<CreateCatchUpRunRequest>,
) -> Result<Response, StatusCode> {
    let user = current_user(&state, &headers).await?;
    if let Some(existing) = state
        .ai_run_by_request(user.id, payload.client_request_id)
        .await
        .map_err(internal_error)?
    {
        return Ok((StatusCode::ACCEPTED, Json(existing)).into_response());
    }
    let mut thread = state
        .ai_thread(user.id, thread_id)
        .await
        .map_err(internal_error)?
        .ok_or(StatusCode::NOT_FOUND)?;
    let room = state
        .room(payload.room_id)
        .await
        .ok_or(StatusCode::NOT_FOUND)?;
    let conversation = state
        .conversation_summary(user.id, payload.room_id)
        .await
        .map_err(internal_error)?
        .ok_or(StatusCode::FORBIDDEN)?;
    require_room_password(&room, &headers)?;
    let Some(window) = state
        .catch_up_window(payload.room_id, user.id)
        .await
        .map_err(internal_error)?
    else {
        return Ok(StatusCode::NO_CONTENT.into_response());
    };
    let model = state
        .resolve_ai_model(payload.model_option_id, thread.thinking_enabled)
        .await
        .map_err(internal_error)?
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    if !state
        .check_action_cooldown(
            payload.room_id,
            user.id,
            thread_id,
            state.ai_suggest_cooldown(),
        )
        .await
    {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }
    if thread.room_id != Some(payload.room_id) || thread.title == DEFAULT_TITLE {
        let title = if thread.title == DEFAULT_TITLE {
            format!("{} · 未读总结", conversation.title)
        } else {
            thread.title.clone()
        };
        thread = state
            .update_ai_thread(
                user.id,
                thread.id,
                &title,
                Some(payload.room_id),
                thread.thinking_enabled,
            )
            .await
            .map_err(internal_error)?
            .ok_or(StatusCode::NOT_FOUND)?;
    }
    let admission = state
        .admit_ai(AiAdmissionRequest {
            user_id: user.id,
            room_id: Some(payload.room_id),
            feature: "catch_up",
            model: &model,
            reserved_tokens: window.unread_message_count.min(500).saturating_mul(64),
        })
        .await
        .map_err(|error| error.status())?;
    let outcome = match state
        .create_catch_up_run(
            user.id,
            thread.id,
            NewCatchUpRun {
                room_id: payload.room_id,
                client_request_id: payload.client_request_id,
                source: CatchUpRunSource {
                    after_message_id: window.after_message_id,
                    through_message_id: window.through_message_id,
                    message_count: window.unread_message_count,
                },
                model: &model,
                admission,
            },
        )
        .await
    {
        Ok(outcome) => outcome,
        Err(error) => {
            let _ = state.discard_ai_admission(admission.id).await;
            return Err(internal_error(error));
        }
    };
    let run = match outcome {
        CreateRunOutcome::Created(run) => run,
        CreateRunOutcome::Existing(run) => {
            let _ = state.discard_ai_admission(admission.id).await;
            run
        }
        CreateRunOutcome::Busy => {
            let _ = state.discard_ai_admission(admission.id).await;
            return Err(StatusCode::CONFLICT);
        }
    };
    spawn_run(state, run.id);
    Ok((StatusCode::ACCEPTED, Json(run)).into_response())
}
