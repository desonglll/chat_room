use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};

use uuid::Uuid;

use crate::conversations::models::{ConversationSummary, UpdateConversationAliasRequest};
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

#[utoipa::path(
    put,
    path = "/api/conversations/{room_id}/alias",
    params(("room_id" = Uuid, Path, description = "Conversation room ID")),
    request_body = UpdateConversationAliasRequest,
    responses(
        (status = 200, description = "Updated private conversation alias", body = ConversationSummary),
        (status = 400, description = "Alias is invalid"),
        (status = 401, description = "Missing or expired session"),
        (status = 404, description = "Conversation is not active for this account")
    )
)]
pub async fn update_conversation_alias(
    State(state): State<SharedState>,
    Path(room_id): Path<Uuid>,
    headers: HeaderMap,
    Json(payload): Json<UpdateConversationAliasRequest>,
) -> Result<Json<ConversationSummary>, StatusCode> {
    let token = bearer_token(&headers)?;
    let user = state
        .session_user(token)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::UNAUTHORIZED)?;
    let alias = payload.alias.trim();
    if alias.chars().count() > 64 || alias.chars().any(char::is_control) {
        return Err(StatusCode::BAD_REQUEST);
    }
    let updated = state
        .set_conversation_alias(user.id, room_id, alias)
        .await
        .map_err(|error| {
            tracing::error!("update conversation alias failed: {error}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    if !updated {
        return Err(StatusCode::NOT_FOUND);
    }
    state
        .conversation_summary(user.id, room_id)
        .await
        .map_err(|error| {
            tracing::error!("load updated conversation alias failed: {error}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}
