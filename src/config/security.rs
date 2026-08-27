use serde::Deserialize;

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct SecurityConfig {
    /// Exact browser origins allowed to call the API cross-origin.
    pub cors_allowed_origins: Vec<String>,
    /// Trust proxy IP headers only behind a sanitizing reverse proxy.
    pub trust_proxy_headers: bool,
}

pub(crate) fn is_exact_origin(origin: &str) -> bool {
    let Ok(uri) = origin.parse::<axum::http::Uri>() else {
        return false;
    };
    let (Some(scheme @ ("http" | "https")), Some(authority)) = (uri.scheme_str(), uri.authority())
    else {
        return false;
    };
    origin == format!("{scheme}://{authority}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_only_exact_http_origins() {
        assert!(is_exact_origin("https://chat.example.com"));
        assert!(is_exact_origin("http://localhost:5173"));
        assert!(!is_exact_origin("*"));
        assert!(!is_exact_origin("https://chat.example.com/path"));
        assert!(!is_exact_origin("https://chat.example.com/"));
        assert!(!is_exact_origin("javascript:alert(1)"));
    }
}
