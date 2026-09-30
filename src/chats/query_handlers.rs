//! Viewer-aware chat discovery and detail queries.

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::Response,
};
use serde::Deserialize;
use uuid::Uuid;

use super::ApiDialect;
use crate::state::SharedState;
use crate::user_handlers::optional_bearer_token;

#[derive(Deserialize)]
pub struct ListQuery {
    /// Exact title filter. Accepted under its pre-TG-006 name `name` as well, so the frozen
    /// clients keep working on both paths.
    #[serde(alias = "name")]
    pub title: Option<String>,
}

#[utoipa::path(
    get,
    path = "/api/chats",
    params(("title" = Option<String>, Query, description = "Filter by exact chat title")),
    responses((status = 200, description = "Matching chats", body = Vec<Chat>))
)]
pub async fn list_chats(
    State(state): State<SharedState>,
    Query(query): Query<ListQuery>,
    dialect: ApiDialect,
    headers: HeaderMap,
) -> Result<Response, StatusCode> {
    let direct_ids = state
        .direct_room_ids()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let mut chats = state.list_chats(query.title.as_deref()).await;
    chats.retain(|chat| !direct_ids.contains(&chat.id));
    let user = if let Some(token) = optional_bearer_token(&headers) {
        state
            .session_user(token)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    } else {
        None
    };
    let Some(user) = user else {
        return Ok(dialect.chats(Vec::new()));
    };
    state
        .decorate_chats_for_user(&mut chats, user.id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    chats.retain(|chat| chat.membership_status.as_deref() == Some("active"));
    Ok(dialect.chats(chats))
}

#[utoipa::path(
    get,
    path = "/api/chats/discover",
    params(("title" = Option<String>, Query, description = "Filter by exact chat title")),
    responses((status = 200, description = "Discoverable public chats", body = Vec<Chat>))
)]
pub async fn discover_chats(
    State(state): State<SharedState>,
    Query(query): Query<ListQuery>,
    dialect: ApiDialect,
    headers: HeaderMap,
) -> Result<Response, StatusCode> {
    let direct_ids = state
        .direct_room_ids()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let mut chats = state.list_chats(query.title.as_deref()).await;
    chats.retain(|chat| !direct_ids.contains(&chat.id) && !chat.has_password);

    if let Some(token) = optional_bearer_token(&headers) {
        if let Some(user) = state
            .session_user(token)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        {
            state
                .decorate_chats_for_user(&mut chats, user.id)
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            chats.retain(|chat| chat.membership_status.as_deref() != Some("active"));
        }
    }
    Ok(dialect.chats(chats))
}

#[utoipa::path(
    get,
    path = "/api/chats/{id}",
    params(("id" = Uuid, description = "Chat id")),
    responses(
        (status = 200, description = "Chat found", body = Chat),
        (status = 404, description = "Chat not found")
    )
)]
pub async fn get_chat(
    State(state): State<SharedState>,
    Path(id): Path<Uuid>,
    dialect: ApiDialect,
    headers: HeaderMap,
) -> Result<Response, StatusCode> {
    let mut chat = state.chat(id).await.ok_or(StatusCode::NOT_FOUND)?;
    chat.membership_status = None;
    chat.membership_role = None;
    let user = if let Some(token) = optional_bearer_token(&headers) {
        state
            .session_user(token)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    } else {
        None
    };
    if let Some(user) = &user {
        if let Some((status, role)) = state
            .membership_identity(id, user.id)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        {
            chat.membership_status = Some(status);
            chat.membership_role = Some(role);
        }
    }
    let direct = state
        .is_direct_chat(id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if direct {
        if chat.membership_status.as_deref() != Some("active") {
            return Err(StatusCode::NOT_FOUND);
        }
        let user = user.ok_or(StatusCode::NOT_FOUND)?;
        let conversation = state
            .conversation_summary(user.id, id)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
            .ok_or(StatusCode::NOT_FOUND)?;
        chat.title = conversation.title;
        chat.avatar_emoji = conversation.avatar_emoji;
        chat.description = conversation.description;
    }
    Ok(dialect.chat(chat))
}
