use std::{str::FromStr, time::Duration};

use axum::{extract::State, http::HeaderMap, http::StatusCode, Json};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use uuid::Uuid;

use super::models::{
    GlobalMessageSearchPage, SearchContentType, SearchCursor, VisibleMessageSearch,
};
use crate::{state::SharedState, user_handlers::bearer_token};

const DEFAULT_LIMIT: i64 = 30;
const MAX_LIMIT: i64 = 50;
const SEARCH_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Debug, Deserialize)]
pub struct GlobalMessageSearchParams {
    q: String,
    room_id: Option<Uuid>,
    sender_id: Option<Uuid>,
    from: Option<DateTime<Utc>>,
    to: Option<DateTime<Utc>>,
    #[serde(default)]
    content_type: SearchContentType,
    cursor: Option<String>,
    limit: Option<i64>,
}

#[utoipa::path(
    get,
    path = "/api/messages/search",
    params(
        ("q" = String, Query, description = "Text to find across visible conversations"),
        ("room_id" = Option<Uuid>, Query, description = "Restrict results to one conversation"),
        ("sender_id" = Option<Uuid>, Query, description = "Restrict results to one sender"),
        ("from" = Option<DateTime<Utc>>, Query, description = "Inclusive lower timestamp bound"),
        ("to" = Option<DateTime<Utc>>, Query, description = "Inclusive upper timestamp bound"),
        ("content_type" = Option<SearchContentType>, Query, description = "all, text, file, image, video, or audio"),
        ("cursor" = Option<String>, Query, description = "Exclusive stable result cursor"),
        ("limit" = Option<i64>, Query, description = "Results to return (1-50)")
    ),
    responses(
        (status = 200, description = "Messages visible to the current account", body = GlobalMessageSearchPage),
        (status = 400, description = "Invalid search query or cursor"),
        (status = 401, description = "Missing or expired session"),
        (status = 504, description = "Search exceeded its execution deadline")
    )
)]
pub async fn search_visible_messages(
    State(state): State<SharedState>,
    axum::extract::Query(params): axum::extract::Query<GlobalMessageSearchParams>,
    headers: HeaderMap,
) -> Result<Json<GlobalMessageSearchPage>, StatusCode> {
    let user = state
        .session_user(bearer_token(&headers)?)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::UNAUTHORIZED)?;
    let search = normalize(params)?;
    tokio::time::timeout(
        SEARCH_TIMEOUT,
        state.search_visible_messages(user.id, &search),
    )
    .await
    .map_err(|_| StatusCode::GATEWAY_TIMEOUT)?
    .map(Json)
    .map_err(|error| {
        tracing::error!("global message search failed: {error}");
        StatusCode::INTERNAL_SERVER_ERROR
    })
}

fn normalize(params: GlobalMessageSearchParams) -> Result<VisibleMessageSearch, StatusCode> {
    let text = params.q.trim();
    let limit = params.limit.unwrap_or(DEFAULT_LIMIT);
    if text.is_empty()
        || text.chars().count() > 200
        || !(1..=MAX_LIMIT).contains(&limit)
        || params
            .from
            .zip(params.to)
            .is_some_and(|(from, to)| from > to)
    {
        return Err(StatusCode::BAD_REQUEST);
    }
    let cursor = params
        .cursor
        .as_deref()
        .map(SearchCursor::from_str)
        .transpose()
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    Ok(VisibleMessageSearch {
        text: text.to_owned(),
        room_id: params.room_id,
        sender_id: params.sender_id,
        from: params.from,
        to: params.to,
        content_type: params.content_type,
        cursor,
        limit,
    })
}
