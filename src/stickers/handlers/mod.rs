//! HTTP translation for the sticker domain. Handlers authenticate, parse, call one
//! `AppState` method and map the result; the rules live in the store modules.

pub mod library;
pub mod messages;
pub mod sets;

use axum::http::HeaderMap;

use super::errors::StickerError;
use crate::models::User;
use crate::state::SharedState;
use crate::user_handlers::bearer_token;

/// The account behind the request's bearer session.
pub(crate) async fn session_account(
    state: &SharedState,
    headers: &HeaderMap,
) -> Result<User, StickerError> {
    let token = bearer_token(headers)?;
    state
        .session_user(token)
        .await?
        .ok_or(StickerError::Unauthorized)
}
