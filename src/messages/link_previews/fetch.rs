//! TG-408: one bounded, SSRF-checked fetch of a page's head. Each hop: check the URL, resolve
//! the host once, check every answer, then connect to that checked address only (reqwest's
//! `resolve` pin), with redirects followed by hand so each new hop is checked the same way.
//! No proxy from the environment is used, the body is cut at `max_bytes`, and the whole fetch
//! has one deadline. TG-1209: the card's image is fetched through the same hop loop
//! (`fetch_image`), so there is exactly one outbound path and one guard.

use std::future::Future;
use std::io;
use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

use reqwest::{redirect, Url};

use super::parse::parse_preview;
use super::ssrf::{check_url, pick_address, Policy, Refusal};
use super::LinkPreview;

pub const MAX_REDIRECTS: usize = 3;
pub const MAX_BYTES: usize = 256 * 1024;
pub const TIMEOUT: Duration = Duration::from_secs(5);
/// TG-1209: a card image larger than this is dropped, never truncated.
pub const MAX_IMAGE_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FetchError {
    Refused(Refusal),
    TooManyRedirects,
    Status(u16),
    NotHtml,
    /// TG-1209: not a PNG, JPEG, GIF or WebP image (judged by its bytes, not its header).
    NotImage,
    TooLarge,
    Network,
    Timeout,
}

/// Fetch `start` and extract its preview. `resolve` is the DNS lookup (production:
/// `tokio::net::lookup_host`); it is called exactly once per hop.
pub async fn fetch_preview<R, F>(
    start: Url,
    policy: &Policy,
    resolve: R,
) -> Result<Option<LinkPreview>, FetchError>
where
    R: Fn(String, u16) -> F,
    F: Future<Output = io::Result<Vec<SocketAddr>>>,
{
    let page = fetch_bounded(start, policy, &resolve, &HTML).await?;
    Ok(parse_preview(
        &String::from_utf8_lossy(&page.body),
        &page.url,
    ))
}

/// TG-1209: a card's image, fetched by the server under the same policy as its page so the
/// browser never contacts the third-party host. The type comes from the bytes (`sniff_image`),
/// not from the response header; SVG and anything else is refused.
pub async fn fetch_image<R, F>(
    start: Url,
    policy: &Policy,
    resolve: R,
) -> Result<FetchedImage, FetchError>
where
    R: Fn(String, u16) -> F,
    F: Future<Output = io::Result<Vec<SocketAddr>>>,
{
    let image = fetch_bounded(start, policy, &resolve, &IMAGE).await?;
    let content_type = sniff_image(&image.body).ok_or(FetchError::NotImage)?;
    Ok(FetchedImage {
        content_type,
        bytes: image.body,
    })
}

#[derive(Debug)]
pub struct FetchedImage {
    pub content_type: &'static str,
    pub bytes: Vec<u8>,
}

/// The image type by magic number. Only raster formats every browser decodes safely.
pub fn sniff_image(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("image/jpeg")
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some("image/gif")
    } else if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("image/webp")
    } else {
        None
    }
}

/// What one kind of fetch asks for and accepts.
struct Kind {
    accept: &'static str,
    type_ok: fn(&str) -> bool,
    wrong_type: FetchError,
    max_bytes: usize,
    /// Cut the body at `max_bytes` (a page's head is enough) instead of failing.
    truncate: bool,
}

const HTML: Kind = Kind {
    accept: "text/html,application/xhtml+xml",
    type_ok: |value| value.starts_with("text/html") || value.starts_with("application/xhtml+xml"),
    wrong_type: FetchError::NotHtml,
    max_bytes: MAX_BYTES,
    truncate: true,
};

const IMAGE: Kind = Kind {
    accept: "image/png,image/jpeg,image/gif,image/webp",
    type_ok: |value| value.starts_with("image/"),
    wrong_type: FetchError::NotImage,
    max_bytes: MAX_IMAGE_BYTES,
    truncate: false,
};

struct Body {
    url: Url,
    body: Vec<u8>,
}

async fn fetch_bounded<R, F>(
    start: Url,
    policy: &Policy,
    resolve: &R,
    kind: &Kind,
) -> Result<Body, FetchError>
where
    R: Fn(String, u16) -> F,
    F: Future<Output = io::Result<Vec<SocketAddr>>>,
{
    tokio::time::timeout(TIMEOUT, fetch_hops(start, policy, resolve, kind))
        .await
        .map_err(|_| FetchError::Timeout)?
}

async fn fetch_hops<R, F>(
    start: Url,
    policy: &Policy,
    resolve: &R,
    kind: &Kind,
) -> Result<Body, FetchError>
where
    R: Fn(String, u16) -> F,
    F: Future<Output = io::Result<Vec<SocketAddr>>>,
{
    let mut url = start;
    for _ in 0..=MAX_REDIRECTS {
        check_url(&url, policy).map_err(FetchError::Refused)?;
        let host = url.host_str().unwrap_or_default().to_string();
        let port = url.port_or_known_default().unwrap_or(80);
        let literal = host
            .trim_start_matches('[')
            .trim_end_matches(']')
            .parse::<IpAddr>()
            .ok();
        let answers = match literal {
            Some(ip) => vec![SocketAddr::new(ip, port)],
            None => resolve(host.clone(), port)
                .await
                .map_err(|_| FetchError::Refused(Refusal::Unresolvable))?,
        };
        let address = pick_address(&answers, policy).map_err(FetchError::Refused)?;
        let mut builder = reqwest::Client::builder()
            .no_proxy()
            .redirect(redirect::Policy::none())
            .timeout(TIMEOUT)
            .user_agent("Mozilla/5.0 (compatible; chat_room-link-preview/1.0)");
        if literal.is_none() {
            builder = builder.resolve(&host, address);
        }
        let client = builder.build().map_err(|_| FetchError::Network)?;
        let mut response = client
            .get(url.clone())
            .header(reqwest::header::ACCEPT, kind.accept)
            .send()
            .await
            .map_err(|_| FetchError::Network)?;
        let status = response.status();
        if status.is_redirection() {
            let location = response
                .headers()
                .get(reqwest::header::LOCATION)
                .and_then(|value| value.to_str().ok())
                .ok_or(FetchError::Status(status.as_u16()))?;
            url = url
                .join(location)
                .map_err(|_| FetchError::Status(status.as_u16()))?;
            continue;
        }
        if !status.is_success() {
            return Err(FetchError::Status(status.as_u16()));
        }
        let type_ok = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| (kind.type_ok)(&value.to_ascii_lowercase()));
        if !type_ok {
            return Err(kind.wrong_type.clone());
        }
        if !kind.truncate
            && response
                .content_length()
                .is_some_and(|length| length > kind.max_bytes as u64)
        {
            return Err(FetchError::TooLarge);
        }
        let mut body = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|_| FetchError::Network)? {
            if !kind.truncate && body.len() + chunk.len() > kind.max_bytes {
                return Err(FetchError::TooLarge);
            }
            let room = kind.max_bytes - body.len();
            body.extend_from_slice(&chunk[..chunk.len().min(room)]);
            if body.len() >= kind.max_bytes {
                break; // The head (where the meta tags live) is what matters; stop reading.
            }
        }
        return Ok(Body { url, body });
    }
    Err(FetchError::TooManyRedirects)
}

/// The production resolver.
pub async fn system_resolve(host: String, port: u16) -> io::Result<Vec<SocketAddr>> {
    Ok(tokio::net::lookup_host((host.as_str(), port))
        .await?
        .collect())
}
