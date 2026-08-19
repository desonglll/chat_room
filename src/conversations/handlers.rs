use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    Json,
};

use crate::conversations::models::ConversationSummary;
use crate::state::SharedState;
use crate::user_handlers::bearer_token;

#[utoipa::path(
    get,
    path = "/api/conversations",
    responses(
        (status = 200, description = "Unified group and direct conversations", body = [ConversationSummary]),
        (status = 401, description = "Missing or expired session")
    )
)]
pub async fn list_conversations(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> Result<Json<Vec<ConversationSummary>>, StatusCode> {
    let token = bearer_token(&headers)?;
    let user = state
        .session_user(token)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::UNAUTHORIZED)?;
    state
        .conversation_summaries(user.id)
        .await
        .map(Json)
        .map_err(|error| {
            tracing::error!("list conversations failed: {error}");
            StatusCode::INTERNAL_SERVER_ERROR
        })
}
