//! One durable server-sent event stream per AI run.

use std::{convert::Infallible, time::Duration};

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::sse::{Event, KeepAlive, Sse},
};
use futures_util::{stream, Stream};
use uuid::Uuid;

use super::{handlers::current_user, models::AiThreadMessage};
use crate::{cache::CachedAiAnswer, state::SharedState};

const STREAM_REFRESH: Duration = Duration::from_millis(100);
const DATABASE_REFRESH_TICKS: u64 = 10;

#[utoipa::path(
    get,
    path = "/api/ai/runs/{id}/events",
    params(("id" = Uuid, Path, description = "AI run id")),
    responses(
        (status = 200, description = "Live AI answer revisions as server-sent events"),
        (status = 404, description = "AI run not found")
    )
)]
pub async fn stream_run_events(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(run_id): Path<Uuid>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, StatusCode> {
    let user = current_user(&state, &headers).await?;
    let message = state
        .ai_run_message(user.id, run_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    let cursor = RunEventCursor {
        state,
        user_id: user.id,
        run_id,
        message,
        first: true,
        ticks: 0,
    };
    let events = stream::unfold(cursor, next_event);
    Ok(Sse::new(events).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive"),
    ))
}

struct RunEventCursor {
    state: SharedState,
    user_id: Uuid,
    run_id: Uuid,
    message: AiThreadMessage,
    first: bool,
    ticks: u64,
}

async fn next_event(
    mut cursor: RunEventCursor,
) -> Option<(Result<Event, Infallible>, RunEventCursor)> {
    if is_terminal(&cursor.message.status) && !cursor.first {
        return None;
    }
    loop {
        if !cursor.first {
            tokio::time::sleep(STREAM_REFRESH).await;
        }
        cursor.ticks += 1;
        let previous = (cursor.message.revision, cursor.message.status.clone());
        let cache_hit = refresh_from_redis(&cursor.state, &mut cursor.message).await;
        if (!cache_hit && cursor.ticks.is_multiple_of(DATABASE_REFRESH_TICKS)) || cursor.first {
            if let Ok(Some(message)) = cursor
                .state
                .ai_run_message(cursor.user_id, cursor.run_id)
                .await
            {
                cursor.message = message;
                let _ = refresh_from_redis(&cursor.state, &mut cursor.message).await;
            }
        }
        let changed = previous != (cursor.message.revision, cursor.message.status.clone());
        if cursor.first || changed {
            cursor.first = false;
            let event = Event::default()
                .event("message")
                .json_data(&cursor.message)
                .unwrap_or_else(|_| Event::default().event("error").data("serialization failed"));
            return Some((Ok(event), cursor));
        }
    }
}

async fn refresh_from_redis(state: &SharedState, message: &mut AiThreadMessage) -> bool {
    let Some(cache) = state.redis_cache() else {
        return false;
    };
    match cache.ai_answer(message.id).await {
        Ok(Some(answer)) => {
            apply_cached_answer(message, answer);
            true
        }
        Ok(None) => false,
        Err(error) => {
            tracing::warn!(message_id = %message.id, "read streamed AI answer failed: {error:#}");
            false
        }
    }
}

fn apply_cached_answer(message: &mut AiThreadMessage, answer: CachedAiAnswer) {
    message.content = answer.content;
    message.context_message_count = Some(answer.context_message_count);
    message.revision = answer.revision;
    message.status = answer.status;
    message.updated_at = answer.updated_at;
}

fn is_terminal(status: &str) -> bool {
    matches!(status, "completed" | "failed")
}
