//! Browser client assets built by Vite from build.rs and embedded in the binary.

use axum::{http::header, response::IntoResponse};

const INDEX_HTML: &str = include_str!(concat!(env!("OUT_DIR"), "/web/index.html"));
const APP_CSS: &str = include_str!(concat!(env!("OUT_DIR"), "/web/assets/app.css"));
const APP_JS: &str = include_str!(concat!(env!("OUT_DIR"), "/web/assets/app.js"));
const ADMIN_DASHBOARD_JS: &str =
    include_str!(concat!(env!("OUT_DIR"), "/web/assets/AdminDashboard.js"));
const JSZIP_JS: &str = include_str!(concat!(env!("OUT_DIR"), "/web/assets/jszip.min.js"));
const FAVICON: &str = include_str!(concat!(env!("OUT_DIR"), "/web/favicon.svg"));
const ICON_SPRITE: &str = include_str!(concat!(env!("OUT_DIR"), "/web/icons/icon-sprite.svg"));
const ECHO_GATE: &str = include_str!(concat!(env!("OUT_DIR"), "/web/brand/echo-gate.svg"));
const EMOJI_DATA_ZH: &str = include_str!(concat!(env!("OUT_DIR"), "/web/emoji-data-zh.json"));

pub async fn index() -> impl IntoResponse {
    asset("text/html; charset=utf-8", "no-cache", INDEX_HTML)
}

pub async fn stylesheet() -> impl IntoResponse {
    asset("text/css; charset=utf-8", "no-cache", APP_CSS)
}

pub async fn app_script() -> impl IntoResponse {
    asset("text/javascript; charset=utf-8", "no-cache", APP_JS)
}

pub async fn admin_dashboard_script() -> impl IntoResponse {
    asset(
        "text/javascript; charset=utf-8",
        "no-cache",
        ADMIN_DASHBOARD_JS,
    )
}

pub async fn jszip_script() -> impl IntoResponse {
    asset("text/javascript; charset=utf-8", "no-cache", JSZIP_JS)
}

pub async fn favicon() -> impl IntoResponse {
    asset("image/svg+xml", "public, max-age=86400", FAVICON)
}

pub async fn icon_sprite() -> impl IntoResponse {
    asset("image/svg+xml", "public, max-age=86400", ICON_SPRITE)
}

pub async fn echo_gate() -> impl IntoResponse {
    asset("image/svg+xml", "public, max-age=86400", ECHO_GATE)
}

pub async fn emoji_data_zh() -> impl IntoResponse {
    asset("application/json", "public, max-age=86400", EMOJI_DATA_ZH)
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
