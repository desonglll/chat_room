//! Browser client assets built by Vite from build.rs and embedded in the binary.

use axum::{
    extract::Path,
    http::{header, StatusCode},
    response::{IntoResponse, Response},
};

const INDEX_HTML: &str = include_str!(concat!(env!("OUT_DIR"), "/web/index.html"));
include!(concat!(env!("OUT_DIR"), "/web_assets.rs"));

pub async fn index() -> impl IntoResponse {
    asset_bytes(
        "text/html; charset=utf-8",
        "no-cache",
        INDEX_HTML.as_bytes(),
    )
}

pub async fn bundled_asset(Path(path): Path<String>) -> Response {
    named_asset(
        &format!("assets/{path}"),
        "public, max-age=31536000, immutable",
    )
}

pub async fn favicon() -> Response {
    named_asset("favicon.svg", "public, max-age=86400")
}

pub async fn icon_sprite() -> Response {
    named_asset("icons/icon-sprite.svg", "public, max-age=86400")
}

pub async fn echo_gate() -> Response {
    named_asset("brand/echo-gate.svg", "public, max-age=86400")
}

pub async fn emoji_data_zh() -> Response {
    named_asset("emoji-data-zh.json", "public, max-age=86400")
}

fn named_asset(name: &str, cache_control: &'static str) -> Response {
    GENERATED_ASSETS
        .iter()
        .find(|(asset_name, _, _)| *asset_name == name)
        .map(|(_, content_type, source)| asset_bytes(content_type, cache_control, source))
        .unwrap_or_else(|| StatusCode::NOT_FOUND.into_response())
}

fn asset_bytes(
    content_type: &'static str,
    cache_control: &'static str,
    source: &'static [u8],
) -> Response {
    (
        [
            (header::CONTENT_TYPE, content_type),
            (header::CACHE_CONTROL, cache_control),
        ],
        source,
    )
        .into_response()
}
