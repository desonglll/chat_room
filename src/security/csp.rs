//! TG-1209: the Content-Security-Policy, built once at startup. `img-src` stays this origin
//! (plus `data:`/`blob:`) with exactly one addition: the host of the configured map tile
//! template (TG-407, D-010), because Leaflet draws tiles as `<img>` straight from that server.
//! Link-card images are not an exception — the server fetches them and serves them same-origin
//! (`messages::link_previews::images`), so no third-party image host is ever allowed.

use axum::http::HeaderValue;
use reqwest::Url;

const BEFORE_IMG: &str =
    "default-src 'self'; base-uri 'self'; form-action 'self'; frame-ancestors 'none'; img-src 'self' data: blob:";
const AFTER_IMG: &str = "; media-src 'self' blob:; object-src 'none'; script-src 'self'; style-src 'self' 'unsafe-inline'; connect-src 'self' ws: wss:";

/// The policy for a deployment whose map tiles come from `tile_url`.
pub fn content_security_policy(tile_url: &str) -> HeaderValue {
    let tiles = tile_image_source(tile_url)
        .map(|source| format!(" {source}"))
        .unwrap_or_default();
    HeaderValue::from_str(&format!("{BEFORE_IMG}{tiles}{AFTER_IMG}"))
        .expect("the CSP is built from checked ASCII only")
}

/// The CSP source that admits the tiles of `template`, or `None` when nothing extra is needed
/// (a same-origin path) or the template's host cannot be expressed safely. `{s}` as the first
/// host label (Leaflet's subdomain rotation, e.g. `{s}.tile.example.org`) becomes `*.` — the
/// only wildcard ever produced. The result is rebuilt from the parsed URL and limited to
/// `[a-z0-9.-]` hosts, so a configured value cannot inject other CSP directives.
pub fn tile_image_source(template: &str) -> Option<String> {
    let template = template.trim();
    let (scheme, rest) = template.split_once("://")?;
    if !matches!(scheme, "http" | "https") {
        return None;
    }
    let authority = rest.split('/').next().unwrap_or_default();
    let (wildcard, authority) = match authority.strip_prefix("{s}.") {
        Some(remainder) => (true, remainder),
        None => (false, authority),
    };
    if authority.contains(['{', '}']) {
        tracing::warn!(
            "map.tile_url has a placeholder in its host; tiles will be blocked by the CSP"
        );
        return None;
    }
    let url = Url::parse(&format!("{scheme}://{authority}/")).ok()?;
    let host = url.host_str()?;
    let safe = |c: char| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.' || c == '-';
    if host.is_empty() || !host.chars().all(safe) || !url.username().is_empty() {
        return None;
    }
    let port = url
        .port()
        .map(|port| format!(":{port}"))
        .unwrap_or_default();
    let star = if wildcard { "*." } else { "" };
    Some(format!("{scheme}://{star}{host}{port}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_osm_template_admits_exactly_its_host() {
        let default = crate::config::MapConfig::default().tile_url;
        assert_eq!(
            tile_image_source(&default).as_deref(),
            Some("https://tile.openstreetmap.org")
        );
        let csp = content_security_policy(&default);
        let csp = csp.to_str().unwrap();
        assert!(
            csp.contains("img-src 'self' data: blob: https://tile.openstreetmap.org;"),
            "{csp}"
        );
        assert_eq!(csp.matches("img-src").count(), 1);
    }

    #[test]
    fn subdomains_ports_and_same_origin_paths() {
        assert_eq!(
            tile_image_source("https://{s}.tiles.example.org/{z}/{x}/{y}.png").as_deref(),
            Some("https://*.tiles.example.org")
        );
        assert_eq!(
            tile_image_source("http://Tiles.Example.org:8081/{z}/{x}/{y}.png?key=1").as_deref(),
            Some("http://tiles.example.org:8081")
        );
        assert_eq!(tile_image_source("/tiles/{z}/{x}/{y}.png"), None);
        let local = content_security_policy("/tiles/{z}/{x}/{y}.png");
        assert!(local
            .to_str()
            .unwrap()
            .contains("img-src 'self' data: blob:;"));
    }

    #[test]
    fn a_hostile_or_odd_template_adds_nothing() {
        for template in [
            "https://evil.example; script-src *;/{z}/{x}/{y}.png",
            "https://a.example/,https://*/{z}/{x}/{y}",
            "https://{z}.example.org/{x}/{y}.png",
            "https://user:pw@tiles.example.org/{z}/{x}/{y}.png",
            "javascript://tiles.example.org/{z}/{x}/{y}",
            "https:///{z}/{x}/{y}",
        ] {
            let source = tile_image_source(template);
            assert!(
                source
                    .as_deref()
                    .is_none_or(|s| !s.contains(';') && !s.contains(',') && !s.contains(' ')),
                "{template}: {source:?}"
            );
            assert!(
                !content_security_policy(template)
                    .to_str()
                    .unwrap()
                    .contains("script-src *"),
                "{template}"
            );
        }
        assert_eq!(
            tile_image_source("https://{z}.example.org/{x}/{y}.png"),
            None
        );
        assert_eq!(
            tile_image_source("https://user:pw@tiles.example.org/{z}/{x}/{y}"),
            None
        );
    }
}
