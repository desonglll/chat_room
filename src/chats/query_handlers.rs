//! Viewer-aware chat discovery and detail queries.

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::Response,
};
use serde::Deserialize;
use uuid::Uuid;

use super::{ApiDialect, ChatType};
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
    responses((status = 200, description = "Chats the viewer is an active member of, of every chat type. A private chat carries its peer's name, avatar and signature as title, avatar_emoji and description. The deprecated /api/rooms alias omits private chats.", body = Vec<Chat>))
)]
pub async fn list_chats(
    State(state): State<SharedState>,
    Query(query): Query<ListQuery>,
    dialect: ApiDialect,
    headers: HeaderMap,
) -> Result<Response, StatusCode> {
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
    let mut chats = state.list_chats(None).await;
    // The frozen clients behind `/api/rooms` list private chats from `/api/conversations`;
    // giving them here too would show every private chat twice. The canonical contract lists
    // every chat the viewer is in, whatever its type (TG-208).
    if dialect == ApiDialect::LegacyRooms {
        chats.retain(|chat| chat.chat_type != ChatType::Private);
    }
    state
        .decorate_chats_for_user(&mut chats, user.id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    chats.retain(|chat| chat.membership_status.as_deref() == Some("active"));
    state
        .present_private_chats(&mut chats, user.id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    // The title filter applies to what the viewer sees, so a private chat is found by its
    // peer's name, not by its internal placeholder title.
    if let Some(title) = query.title.as_deref() {
        chats.retain(|chat| chat.title == title);
    }
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
    let mut chats = state.list_chats(query.title.as_deref()).await;
    // A private chat is never discoverable: nobody but its two participants may join it.
    chats.retain(|chat| chat.chat_type != ChatType::Private && !chat.has_password);

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
    // Refresh the cached projections (type, member count) from the row first; they can
    // change outside the membership handlers (an account deletion cascades).
    state
        .sync_chat_projection(id, false)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
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
    if chat.chat_type == ChatType::Private {
        // A private chat does not exist for anyone but its active participants, and each of
        // them sees it titled after the other one.
        if chat.membership_status.as_deref() != Some("active") {
            return Err(StatusCode::NOT_FOUND);
        }
        let user = user.ok_or(StatusCode::NOT_FOUND)?;
        state
            .present_private_chats(std::slice::from_mut(&mut chat), user.id)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }
    Ok(dialect.chat(chat))
}
