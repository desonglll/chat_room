//! TG-1209: a card's image is fetched by the server (`fetch::fetch_image`, same SSRF policy and
//! hop loop as the page) and served from this origin under an unguessable key — the same
//! capability-URL model as attachments (`/api/attachments/:id?key=`), because an `<img>` cannot
//! send the session header. The browser never contacts the third-party host, so the CSP keeps
//! `img-src` to this origin. The key is stable across re-fetches of the same page, so a card
//! already on screen keeps working when its image is refreshed.

use axum::{
    body::Body,
    extract::{Path, State},
    http::{
        header::{CACHE_CONTROL, CONTENT_DISPOSITION, CONTENT_TYPE},
        HeaderValue, Response, StatusCode,
    },
};
use chrono::Utc;
use uuid::Uuid;

use super::fetch::{self, FetchedImage};
use super::ssrf::Policy;
use crate::state::{with_pool, AppState, SharedState};

/// The same-origin URL a card's `image_url` carries.
pub fn image_path(access_key: Uuid) -> String {
    format!("/api/link-previews/images/{access_key}")
}

/// The page names its image (`source`, an absolute http(s) URL from `parse`); the server fetches
/// it under the same policy as the page. Any refusal or failure just means a card without one.
pub(super) async fn fetch_card_image(
    source: Option<&str>,
    policy: &Policy,
) -> Option<FetchedImage> {
    let source = reqwest::Url::parse(source?).ok()?;
    fetch::fetch_image(source, policy, fetch::system_resolve)
        .await
        .map_err(|error| tracing::debug!("link preview image refused or failed: {error:?}"))
        .ok()
}

impl AppState {
    /// Keep (or, with `None`, drop) the image of the cached page `url`. The key survives an
    /// update, so cards already delivered keep pointing at the right image.
    pub(super) async fn store_preview_image(
        &self,
        url: &str,
        image: Option<&FetchedImage>,
    ) -> Result<Option<Uuid>, sqlx::Error> {
        let Some(image) = image else {
            with_pool!(self, |pool| {
                sqlx::query("DELETE FROM link_preview_images WHERE url = $1")
                    .bind(url)
                    .execute(pool)
                    .await
                    .map(|_| ())
            })?;
            return Ok(None);
        };
        let fresh_key = Uuid::new_v4();
        let key: Uuid = with_pool!(self, |pool| {
            sqlx::query_scalar(
                "INSERT INTO link_preview_images (url, access_key, content_type, data, fetched_at) \
                 VALUES ($1, $2, $3, $4, $5) \
                 ON CONFLICT (url) DO UPDATE SET content_type = excluded.content_type, \
                   data = excluded.data, fetched_at = excluded.fetched_at \
                 RETURNING access_key",
            )
            .bind(url)
            .bind(fresh_key)
            .bind(image.content_type)
            .bind(&image.bytes)
            .bind(Utc::now())
            .fetch_one(pool)
            .await
        })?;
        Ok(Some(key))
    }

    /// The image's type and bytes, if `access_key` names one whose page still previews fine.
    async fn preview_image(
        &self,
        access_key: Uuid,
    ) -> Result<Option<(String, Vec<u8>)>, sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query_as(
                "SELECT images.content_type, images.data FROM link_preview_images AS images \
                 JOIN link_previews AS previews ON previews.url = images.url AND previews.ok \
                 WHERE images.access_key = $1",
            )
            .bind(access_key)
            .fetch_optional(pool)
            .await
        })
    }
}

/// A card image. The key is the capability (like an attachment's); 404 for anything else.
#[utoipa::path(get, path = "/api/link-previews/images/{key}",
    params(("key" = Uuid, description = "The image key from a card's image_url")),
    responses((status = 200, description = "The image (PNG, JPEG, GIF or WebP)"),
        (status = 404, description = "No such image")))]
pub async fn get_link_preview_image(
    State(state): State<SharedState>,
    Path(access_key): Path<Uuid>,
) -> Result<Response<Body>, StatusCode> {
    let (content_type, bytes) = state
        .preview_image(access_key)
        .await
        .map_err(|error| {
            tracing::error!("link preview image query failed: {error}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or(StatusCode::NOT_FOUND)?;
    let mut response = Response::new(Body::from(bytes));
    let headers = response.headers_mut();
    headers.insert(
        CONTENT_TYPE,
        HeaderValue::from_str(&content_type).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
    );
    headers.insert(CONTENT_DISPOSITION, HeaderValue::from_static("inline"));
    headers.insert(
        CACHE_CONTROL,
        HeaderValue::from_static("private, max-age=86400"),
    );
    Ok(response)
}
