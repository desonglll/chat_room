//! Browser client assets built by Vite from build.rs and embedded in the binary.

use axum::{
    extract::Path,
    http::{header, StatusCode},
    response::{IntoResponse, Response},
};

const INDEX_HTML: &str = include_str!(concat!(env!("OUT_DIR"), "/web/index.html"));
const FAVICON: &str = include_str!(concat!(env!("OUT_DIR"), "/web/favicon.svg"));
const ICON_SPRITE: &str = include_str!(concat!(env!("OUT_DIR"), "/web/icons/icon-sprite.svg"));
const ECHO_GATE: &str = include_str!(concat!(env!("OUT_DIR"), "/web/brand/echo-gate.svg"));
const EMOJI_DATA_ZH: &str = include_str!(concat!(env!("OUT_DIR"), "/web/emoji-data-zh.json"));
include!(concat!(env!("OUT_DIR"), "/web_assets.rs"));

pub async fn index() -> impl IntoResponse {
    asset("text/html; charset=utf-8", "no-cache", INDEX_HTML)
}

pub async fn bundled_asset(Path(path): Path<String>) -> Response {
    GENERATED_ASSETS
        .iter()
        .find(|(name, _, _)| *name == path)
        .map(|(_, content_type, source)| asset_bytes(content_type, source))
        .unwrap_or_else(|| StatusCode::NOT_FOUND.into_response())
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

fn asset_bytes(content_type: &'static str, source: &'static [u8]) -> Response {
    (
        [
            (header::CONTENT_TYPE, content_type),
            (header::CACHE_CONTROL, "no-cache"),
        ],
        source,
    )
        .into_response()
}
