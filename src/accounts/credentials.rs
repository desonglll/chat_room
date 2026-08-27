use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use axum::http::{header::AUTHORIZATION, HeaderMap, StatusCode};
use uuid::Uuid;

const MAX_USERNAME_CHARS: usize = 48;
pub(super) const MIN_PASSWORD_CHARS: usize = 8;
pub(super) const MAX_PASSWORD_CHARS: usize = 256;

pub(super) fn normalize_credentials(
    username: String,
    password: String,
) -> Result<(String, String), StatusCode> {
    let username = username.trim().to_string();
    let password_chars = password.chars().count();
    if username.is_empty()
        || username.chars().count() > MAX_USERNAME_CHARS
        || !(MIN_PASSWORD_CHARS..=MAX_PASSWORD_CHARS).contains(&password_chars)
    {
        return Err(StatusCode::BAD_REQUEST);
    }
    Ok((username, password))
}

pub(super) async fn hash_password(password: String) -> Result<String, StatusCode> {
    tokio::task::spawn_blocking(move || {
        let salt = SaltString::encode_b64(Uuid::new_v4().as_bytes())
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        Argon2::default()
            .hash_password(password.as_bytes(), &salt)
            .map(|hash| hash.to_string())
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
    })
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
}

pub(super) async fn password_matches(password: String, encoded_hash: String) -> bool {
    tokio::task::spawn_blocking(move || {
        PasswordHash::new(&encoded_hash).is_ok_and(|hash| {
            Argon2::default()
                .verify_password(password.as_bytes(), &hash)
                .is_ok()
        })
    })
    .await
    .unwrap_or(false)
}

pub(crate) fn bearer_token(headers: &HeaderMap) -> Result<Uuid, StatusCode> {
    optional_bearer_token(headers).ok_or(StatusCode::UNAUTHORIZED)
}

pub(crate) fn optional_bearer_token(headers: &HeaderMap) -> Option<Uuid> {
    headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .and_then(|value| value.parse().ok())
}
