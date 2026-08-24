use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    Json,
};

use crate::admin_system_lock::{require_chat_rooms_unlocked, require_room_unlocked};
use crate::conversations::models::ConversationSummary;
use crate::social::models::FriendRequestPayload;
use crate::state::SharedState;
use crate::user_handlers::bearer_token;

#[utoipa::path(
    post,
    path = "/api/direct-chats",
    request_body = FriendRequestPayload,
    responses(
        (status = 200, description = "Direct conversation", body = ConversationSummary),
        (status = 400, description = "Cannot chat with self"),
        (status = 409, description = "Accepted friendship required")
    )
)]
pub async fn start_direct_chat(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(payload): Json<FriendRequestPayload>,
) -> Result<Json<ConversationSummary>, StatusCode> {
    let token = bearer_token(&headers)?;
    let user = state
        .session_user(token)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::UNAUTHORIZED)?;
    require_chat_rooms_unlocked(&state).await?;
    if user.id == payload.user_id {
        return Err(StatusCode::BAD_REQUEST);
    }
    let room_id = state
        .start_direct_conversation(user.id, payload.user_id)
        .await
        .map_err(|error| match error {
            sqlx::Error::RowNotFound => StatusCode::CONFLICT,
            other => {
                tracing::error!("start direct chat failed: {other}");
                StatusCode::INTERNAL_SERVER_ERROR
            }
        })?;
    require_room_unlocked(&state, room_id).await?;
    state
        .conversation_summary(user.id, room_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .map(Json)
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)
}
