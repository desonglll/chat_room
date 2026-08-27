use std::net::SocketAddr;

use axum::{extract::ConnectInfo, http::HeaderMap};

pub(crate) fn client_address(
    headers: &HeaderMap,
    peer: Option<ConnectInfo<SocketAddr>>,
    trust_proxy_headers: bool,
) -> String {
    if trust_proxy_headers {
        if let Some(address) = forwarded_address(headers) {
            return address;
        }
    }
    peer.map(|ConnectInfo(address)| address.ip().to_string())
        .unwrap_or_else(|| "local-unknown".into())
}

fn forwarded_address(headers: &HeaderMap) -> Option<String> {
    headers
        .get("x-forwarded-for")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(',').next())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| {
            headers
                .get("x-real-ip")
                .and_then(|value| value.to_str().ok())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignores_forwarded_headers_until_explicitly_trusted() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", "203.0.113.7, 10.0.0.1".parse().unwrap());
        let peer = Some(ConnectInfo("127.0.0.1:3000".parse().unwrap()));

        assert_eq!(client_address(&headers, peer, false), "127.0.0.1");
        assert_eq!(client_address(&headers, peer, true), "203.0.113.7");
    }
}
