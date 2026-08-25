use axum::{extract::State, http::HeaderMap, http::StatusCode, Json};

use super::AiModelChoice;
use crate::{state::SharedState, user_handlers::bearer_token};

#[utoipa::path(
    get,
    path = "/api/ai/models",
    responses(
        (status = 200, description = "Selectable AI model endpoints", body = [AiModelChoice]),
        (status = 401, description = "Missing session")
    )
)]
pub async fn list_models(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> Result<Json<Vec<AiModelChoice>>, StatusCode> {
    let token = bearer_token(&headers)?;
    state
        .session_user(token)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::UNAUTHORIZED)?;
    state
        .ai_model_choices()
        .await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}
