use std::net::SocketAddr;

use axum::{extract::ConnectInfo, http::HeaderMap, http::StatusCode};

use crate::{
    security::{client_address, AuthAction},
    state::SharedState,
};

pub(crate) async fn require_auth_capacity(
    state: &SharedState,
    headers: &HeaderMap,
    peer: Option<ConnectInfo<SocketAddr>>,
    action: AuthAction,
    account: &str,
) -> Result<(), StatusCode> {
    let address = client_address(headers, peer, state.trust_proxy_headers());
    state
        .auth_rate_limits()
        .check(action, &address, account)
        .await
        .then_some(())
        .ok_or(StatusCode::TOO_MANY_REQUESTS)
}
