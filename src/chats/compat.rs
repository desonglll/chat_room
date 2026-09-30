//! Which HTTP dialect a request arrived in, and how a chat descriptor is shaped for it.
//!
//! `/api/chats/*` is the contract; `/api/rooms/*` is a deprecated alias onto the *same*
//! handlers (`routes::chat_scoped_routes` builds both from one list). The only observable
//! difference is the chat descriptor's `name` field, so the difference is resolved here, at
//! the serialisation boundary, and nowhere else.

use async_trait::async_trait;
use axum::{
    extract::FromRequestParts,
    http::{request::Parts, Request},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};

use crate::models::{Chat, ChatCompatView};

/// Which spelling of the chat descriptor the caller asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ApiDialect {
    /// `/api/chats/*` — `title` only.
    #[default]
    Chats,
    /// `/api/rooms/*` — `title` plus the deprecated `name`. Removed in M6.
    LegacyRooms,
}

/// Layer on the alias router. Marks the request so the handlers below can answer in the old
/// dialect; it inspects nothing and rewrites nothing.
pub async fn mark_legacy_room_dialect(
    mut request: Request<axum::body::Body>,
    next: Next,
) -> Response {
    request.extensions_mut().insert(ApiDialect::LegacyRooms);
    next.run(request).await
}

#[async_trait]
impl<S> FromRequestParts<S> for ApiDialect
where
    S: Send + Sync,
{
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        Ok(parts
            .extensions
            .get::<ApiDialect>()
            .copied()
            .unwrap_or_default())
    }
}

impl ApiDialect {
    /// One chat, shaped for this dialect.
    pub fn chat(self, chat: Chat) -> Response {
        match self {
            ApiDialect::Chats => Json(chat).into_response(),
            ApiDialect::LegacyRooms => Json(ChatCompatView::from(chat)).into_response(),
        }
    }

    /// Many chats, shaped for this dialect.
    pub fn chats(self, chats: Vec<Chat>) -> Response {
        match self {
            ApiDialect::Chats => Json(chats).into_response(),
            ApiDialect::LegacyRooms => Json(
                chats
                    .into_iter()
                    .map(ChatCompatView::from)
                    .collect::<Vec<_>>(),
            )
            .into_response(),
        }
    }
}

/// The published spec: every annotated operation, plus the deprecated `/api/rooms/*` alias.
///
/// The alias is not annotated on the handlers — utoipa allows one path per operation, and a
/// second `#[utoipa::path]` per handler would be a second thing to keep in sync. Instead each
/// `/api/chats*` path item is cloned onto its `/api/rooms*` twin here and every operation on
/// the clone is marked deprecated. Delete this function in M6 with the alias itself.
pub fn openapi_with_deprecated_chat_alias() -> utoipa::openapi::OpenApi {
    let mut spec = <crate::ApiDoc as utoipa::OpenApi>::openapi();
    let aliases: Vec<(String, utoipa::openapi::PathItem)> = spec
        .paths
        .paths
        .iter()
        .filter_map(|(path, item)| {
            let suffix = path.strip_prefix(super::routes::CHAT_PREFIX)?;
            let mut item = item.clone();
            for operation in item.operations.values_mut() {
                operation.deprecated = Some(utoipa::openapi::Deprecated::True);
                let note = format!(
                    "Deprecated alias of `{}{suffix}`, removed in M6. Chat descriptors on this \
                     path carry the pre-rename `name` field alongside `title`.",
                    super::routes::CHAT_PREFIX
                );
                operation.description = Some(match operation.description.take() {
                    Some(existing) => format!("{note}\n\n{existing}"),
                    None => note,
                });
            }
            Some((
                format!("{}{suffix}", super::routes::DEPRECATED_CHAT_PREFIX),
                item,
            ))
        })
        .collect();
    for (path, item) in aliases {
        spec.paths.paths.insert(path, item);
    }
    spec
}

/// Serve the OpenAPI JSON spec at /api-docs/openapi.json.
pub async fn openapi_json() -> Json<utoipa::openapi::OpenApi> {
    Json(openapi_with_deprecated_chat_alias())
}
