//! HTTP surface of TG-408: the composer's preview while typing, and the sender hiding a card.

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use serde::Deserialize;
use uuid::Uuid;

use super::first_url;
use crate::chats::membership_handlers::session_user;
use crate::models::ChatMessage;
use crate::state::SharedState;

fn internal(error: sqlx::Error) -> StatusCode {
    tracing::error!("link preview query failed: {error}");
    StatusCode::INTERNAL_SERVER_ERROR
}

#[derive(Debug, Deserialize)]
pub struct PreviewQuery {
    url: String,
}

/// The card a link would get, for the composer. 204 when there is none (refused by the SSRF
/// policy, not HTML, no metadata, or previews are off).
#[utoipa::path(get, path = "/api/link-preview", params(("url" = String, Query, description = "An http(s) link")),
    responses((status = 200, description = "The preview", body = super::LinkPreview),
        (status = 204, description = "No preview for this link"),
        (status = 400, description = "Not an http(s) link")))]
pub async fn get_link_preview(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Query(query): Query<PreviewQuery>,
) -> Result<axum::response::Response, StatusCode> {
    use axum::response::IntoResponse;
    session_user(&state, &headers).await?;
    let url = first_url(query.url.trim()).ok_or(StatusCode::BAD_REQUEST)?;
    if !state.config.link_preview.enabled {
        return Ok(StatusCode::NO_CONTENT.into_response());
    }
    Ok(match state.link_preview_for(url).await.map_err(internal)? {
        Some(preview) => Json(preview).into_response(),
        None => StatusCode::NO_CONTENT.into_response(),
    })
}

/// The sender removes the card from their message (before or after it was built).
#[utoipa::path(delete, path = "/api/chats/{id}/messages/{message_id}/link-preview",
    params(("id" = Uuid, description = "Chat id"), ("message_id" = Uuid, description = "Message id")),
    responses((status = 204, description = "Hidden for everyone"),
        (status = 404, description = "Not your message in this chat")))]
pub async fn hide_link_preview(
    State(state): State<SharedState>,
    Path((room_id, message_id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
) -> Result<StatusCode, StatusCode> {
    let user = session_user(&state, &headers).await?;
    if !state
        .hide_link_preview(room_id, user.id, message_id)
        .await
        .map_err(internal)?
    {
        return Err(StatusCode::NOT_FOUND);
    }
    state
        .broadcast(
            room_id,
            ChatMessage::LinkPreviewUpdated {
                message_id,
                preview: None,
            },
        )
        .await;
    Ok(StatusCode::NO_CONTENT)
}
