//! TG-903: `GET /api/friends/statuses` — the contacts list's online / last-seen line.

use axum::{extract::State, http::HeaderMap, http::StatusCode, Json};
use chrono::Utc;

use crate::models::UserStatusEntry;
use crate::state::SharedState;
use crate::user_handlers::bearer_token;

#[utoipa::path(
    get,
    path = "/api/friends/statuses",
    responses(
        (status = 200, description = "One `{user_id, status}` per friend (the `auth_ok.statuses` shape), under the TG-505 last_seen rules"),
        (status = 401, description = "Missing or expired session")
    )
)]
pub async fn friend_statuses(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> Result<Json<Vec<UserStatusEntry>>, StatusCode> {
    let user_id = state
        .session_user(bearer_token(&headers)?)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::UNAUTHORIZED)?
        .id;
    state
        .friend_statuses(user_id, Utc::now())
        .await
        .map(Json)
        .map_err(|error| {
            tracing::error!("friend statuses failed: {error}");
            StatusCode::INTERNAL_SERVER_ERROR
        })
}
