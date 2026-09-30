//! One error type for the sticker domain, translated to HTTP in one place.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};

use super::validation::StickerRejection;

#[derive(Debug)]
pub enum StickerError {
    /// The uploaded file failed validation.
    Rejected(StickerRejection),
    /// A request field is malformed; the code is the wire string.
    Invalid(&'static str),
    Unauthorized,
    Forbidden,
    NotFound(&'static str),
    Conflict(&'static str),
    Unavailable,
    Database(sqlx::Error),
    Storage(anyhow::Error),
}

impl From<sqlx::Error> for StickerError {
    fn from(error: sqlx::Error) -> Self {
        StickerError::Database(error)
    }
}

impl From<StickerRejection> for StickerError {
    fn from(rejection: StickerRejection) -> Self {
        StickerError::Rejected(rejection)
    }
}

impl From<StatusCode> for StickerError {
    fn from(status: StatusCode) -> Self {
        match status {
            StatusCode::UNAUTHORIZED => StickerError::Unauthorized,
            StatusCode::FORBIDDEN => StickerError::Forbidden,
            StatusCode::NOT_FOUND => StickerError::NotFound("not_found"),
            StatusCode::BAD_REQUEST => StickerError::Invalid("bad_request"),
            StatusCode::PAYLOAD_TOO_LARGE => StickerError::Rejected(StickerRejection::FileTooLarge),
            StatusCode::SERVICE_UNAVAILABLE => StickerError::Unavailable,
            _ => StickerError::Storage(anyhow::anyhow!("sticker request failed with {status}")),
        }
    }
}

impl IntoResponse for StickerError {
    fn into_response(self) -> Response {
        let (status, code) = match self {
            StickerError::Rejected(rejection) => (StatusCode::BAD_REQUEST, rejection.code()),
            StickerError::Invalid(code) => (StatusCode::BAD_REQUEST, code),
            StickerError::Unauthorized => (StatusCode::UNAUTHORIZED, "unauthorized"),
            StickerError::Forbidden => (StatusCode::FORBIDDEN, "forbidden"),
            StickerError::NotFound(code) => (StatusCode::NOT_FOUND, code),
            StickerError::Conflict(code) => (StatusCode::CONFLICT, code),
            StickerError::Unavailable => (StatusCode::SERVICE_UNAVAILABLE, "busy"),
            StickerError::Database(error) => {
                tracing::error!("sticker database operation failed: {error}");
                (StatusCode::INTERNAL_SERVER_ERROR, "internal")
            }
            StickerError::Storage(error) => {
                tracing::error!("sticker storage operation failed: {error:#}");
                (StatusCode::INTERNAL_SERVER_ERROR, "internal")
            }
        };
        (status, Json(serde_json::json!({ "error": code }))).into_response()
    }
}
