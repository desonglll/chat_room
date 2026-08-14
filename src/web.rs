//! Browser client assets built by Vite from build.rs and embedded in the binary.

use axum::{http::header, response::IntoResponse};

const INDEX_HTML: &str = include_str!(concat!(env!("OUT_DIR"), "/web/index.html"));
const APP_CSS: &str = include_str!(concat!(env!("OUT_DIR"), "/web/assets/app.css"));
const APP_JS: &str = include_str!(concat!(env!("OUT_DIR"), "/web/assets/app.js"));
const FAVICON: &str = include_str!(concat!(env!("OUT_DIR"), "/web/favicon.svg"));

pub async fn index() -> impl IntoResponse {
    asset("text/html; charset=utf-8", "no-cache", INDEX_HTML)
}

pub async fn stylesheet() -> impl IntoResponse {
    asset("text/css; charset=utf-8", "no-cache", APP_CSS)
}

pub async fn app_script() -> impl IntoResponse {
    asset("text/javascript; charset=utf-8", "no-cache", APP_JS)
}

pub async fn favicon() -> impl IntoResponse {
    asset("image/svg+xml", "public, max-age=86400", FAVICON)
}

fn asset(
    content_type: &'static str,
    cache_control: &'static str,
    source: &'static str,
) -> impl IntoResponse {
    (
        [
            (header::CONTENT_TYPE, content_type),
            (header::CACHE_CONTROL, cache_control),
        ],
        source,
    )
}
