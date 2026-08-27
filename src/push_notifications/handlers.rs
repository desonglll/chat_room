use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    Json,
};
use serde::Serialize;
use utoipa::ToSchema;

use super::models::{
    DeletePushSubscriptionRequest, PushSubscriptionView, SavePushSubscriptionRequest,
};
use crate::{state::SharedState, user_handlers::bearer_token};

#[derive(Serialize, ToSchema)]
pub struct WebPushPublicConfig {
    enabled: bool,
    public_key: Option<String>,
}

fn valid_endpoint(endpoint: &str, allowed_hosts: &[String]) -> bool {
    if endpoint.len() > 2_048 {
        return false;
    }
    reqwest::Url::parse(endpoint).is_ok_and(|url| {
        if url.scheme() != "https" || !url.username().is_empty() || url.password().is_some() {
            return false;
        }
        let Some(host) = url.host_str() else {
            return false;
        };
        allowed_hosts.iter().any(|allowed| {
            host.eq_ignore_ascii_case(allowed)
                || host
                    .to_ascii_lowercase()
                    .ends_with(&format!(".{}", allowed.to_ascii_lowercase()))
        })
    })
}

fn valid_key(value: &str, max_len: usize) -> bool {
    !value.is_empty()
        && value.len() <= max_len
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

async fn user_id(state: &SharedState, headers: &HeaderMap) -> Result<uuid::Uuid, StatusCode> {
    state
        .session_user(bearer_token(headers)?)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .map(|user| user.id)
        .ok_or(StatusCode::UNAUTHORIZED)
}

#[utoipa::path(
    get,
    path = "/api/push/config",
    responses(
        (status = 200, description = "Public Web Push configuration", body = WebPushPublicConfig)
    )
)]
pub async fn public_config(State(state): State<SharedState>) -> Json<WebPushPublicConfig> {
    let config = state.web_push_config();
    Json(WebPushPublicConfig {
        enabled: config.enabled,
        public_key: config.enabled.then(|| config.public_key.clone()),
    })
}

#[utoipa::path(
    post,
    path = "/api/push/subscriptions",
    request_body = SavePushSubscriptionRequest,
    responses(
        (status = 200, description = "Current browser subscription saved", body = PushSubscriptionView),
        (status = 400, description = "Invalid subscription"),
        (status = 401, description = "Missing or expired session"),
        (status = 409, description = "Endpoint belongs to another subscription")
    )
)]
pub async fn save_subscription(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(request): Json<SavePushSubscriptionRequest>,
) -> Result<Json<PushSubscriptionView>, StatusCode> {
    if !state.web_push_config().enabled {
        return Err(StatusCode::NOT_FOUND);
    }
    if !valid_endpoint(
        &request.endpoint,
        &state.web_push_config().allowed_endpoint_hosts,
    ) || !valid_key(&request.keys.p256dh, 256)
        || !valid_key(&request.keys.auth, 128)
    {
        return Err(StatusCode::BAD_REQUEST);
    }
    let user_id = user_id(&state, &headers).await?;
    state
        .save_push_subscription(user_id, &request)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .map(Json)
        .ok_or(StatusCode::CONFLICT)
}

#[utoipa::path(
    delete,
    path = "/api/push/subscriptions",
    request_body = DeletePushSubscriptionRequest,
    responses(
        (status = 204, description = "Current browser subscription removed"),
        (status = 400, description = "Invalid endpoint"),
        (status = 401, description = "Missing or expired session")
    )
)]
pub async fn delete_subscription(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(request): Json<DeletePushSubscriptionRequest>,
) -> Result<StatusCode, StatusCode> {
    if !valid_endpoint(
        &request.endpoint,
        &state.web_push_config().allowed_endpoint_hosts,
    ) {
        return Err(StatusCode::BAD_REQUEST);
    }
    let user_id = user_id(&state, &headers).await?;
    state
        .delete_push_subscription(user_id, &request.endpoint)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(StatusCode::NO_CONTENT)
}
