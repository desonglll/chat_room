//! Authentication abuse controls and browser-facing HTTP policy.

mod client_address;
mod csp;
mod headers;
mod rate_limits;

pub(crate) use client_address::client_address;
pub(crate) use csp::content_security_policy;
pub(crate) use headers::{cors_layer, security_headers};
pub(crate) use rate_limits::{AuthAction, AuthRateLimitSnapshot, AuthRateLimits};
