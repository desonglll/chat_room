//! TG-408: one bounded, SSRF-checked fetch of a page's head. Each hop: check the URL, resolve
//! the host once, check every answer, then connect to that checked address only (reqwest's
//! `resolve` pin), with redirects followed by hand so each new hop is checked the same way.
//! No proxy from the environment is used, the body is cut at `max_bytes`, and the whole fetch
//! has one deadline.

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

#[derive(Debug, PartialEq, Eq)]
pub enum FetchError {
    Refused(Refusal),
    TooManyRedirects,
    Status(u16),
    NotHtml,
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
    tokio::time::timeout(TIMEOUT, fetch_hops(start, policy, &resolve))
        .await
        .map_err(|_| FetchError::Timeout)?
}

async fn fetch_hops<R, F>(
    start: Url,
    policy: &Policy,
    resolve: &R,
) -> Result<Option<LinkPreview>, FetchError>
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
            .header(reqwest::header::ACCEPT, "text/html,application/xhtml+xml")
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
        let html_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| {
                let value = value.to_ascii_lowercase();
                value.starts_with("text/html") || value.starts_with("application/xhtml+xml")
            });
        if !html_type {
            return Err(FetchError::NotHtml);
        }
        let mut body = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|_| FetchError::Network)? {
            let room = MAX_BYTES - body.len();
            body.extend_from_slice(&chunk[..chunk.len().min(room)]);
            if body.len() >= MAX_BYTES {
                break; // The head (where the meta tags live) is what matters; stop reading.
            }
        }
        return Ok(parse_preview(&String::from_utf8_lossy(&body), &url));
    }
    Err(FetchError::TooManyRedirects)
}

/// The production resolver.
pub async fn system_resolve(host: String, port: u16) -> io::Result<Vec<SocketAddr>> {
    Ok(tokio::net::lookup_host((host.as_str(), port))
        .await?
        .collect())
}
