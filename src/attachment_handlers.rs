//! Authenticated attachment upload and capability-scoped download endpoints.

use axum::{
    body::Body,
    extract::{Multipart, Path, Query, State},
    http::{
        header::{
            ACCEPT_RANGES, AUTHORIZATION, CACHE_CONTROL, CONTENT_DISPOSITION, CONTENT_LENGTH,
            CONTENT_RANGE, CONTENT_TYPE, RANGE,
        },
        HeaderMap, HeaderValue, Response, StatusCode,
    },
    Json,
};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::message_store::NewAttachment;
use crate::models::{Room, StoredMessage, User};
use crate::state::SharedState;

pub const MAX_UPLOAD_BYTES: usize = 50 * 1024 * 1024;
pub const MULTIPART_BODY_LIMIT: usize = MAX_UPLOAD_BYTES + 1024 * 1024;
const MAX_FILE_NAME_CHARS: usize = 255;

#[derive(Deserialize)]
pub struct AttachmentAccess {
    key: Uuid,
}

#[utoipa::path(
    post,
    path = "/api/rooms/{id}/attachments",
    params(
        ("id" = Uuid, description = "Room id"),
        ("authorization" = String, Header, description = "Bearer session token"),
        ("x-room-password" = Option<String>, Header, description = "Required for private rooms")
    ),
    responses(
        (status = 201, description = "Attachment message created", body = StoredMessage),
        (status = 400, description = "Missing, empty, or invalid file"),
        (status = 401, description = "Invalid account or room credentials"),
        (status = 413, description = "File exceeds 50 MiB")
    )
)]
pub async fn upload_attachment(
    State(state): State<SharedState>,
    Path(room_id): Path<Uuid>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<StoredMessage>), StatusCode> {
    let (room, user) = authorize_upload(&state, room_id, &headers).await?;
    let mut upload = None;

    while let Some(field) = multipart.next_field().await.map_err(|error| {
        tracing::warn!("read attachment multipart field failed: {}", error);
        StatusCode::BAD_REQUEST
    })? {
        if field.name() != Some("file") || upload.is_some() {
            continue;
        }
        let file_name = normalize_file_name(field.file_name().unwrap_or("file"))?;
        let supplied_mime = field.content_type().map(str::to_string);
        let data = field.bytes().await.map_err(|error| {
            tracing::warn!("read attachment bytes failed: {}", error);
            StatusCode::BAD_REQUEST
        })?;
        if data.is_empty() {
            return Err(StatusCode::BAD_REQUEST);
        }
        if data.len() > MAX_UPLOAD_BYTES {
            return Err(StatusCode::PAYLOAD_TOO_LARGE);
        }
        let mime_type = normalized_mime(supplied_mime.as_deref(), &file_name);
        upload = Some(NewAttachment {
            file_name,
            mime_type,
            data: data.to_vec(),
        });
    }

    let upload = upload.ok_or(StatusCode::BAD_REQUEST)?;
    state
        .store_attachment_message(room.id, user.id, &user.username, upload)
        .await
        .map(|message| (StatusCode::CREATED, Json(message)))
        .map_err(|error| {
            tracing::error!("persist attachment message failed: {}", error);
            StatusCode::INTERNAL_SERVER_ERROR
        })
}

#[utoipa::path(
    get,
    path = "/api/attachments/{id}",
    params(
        ("id" = Uuid, description = "Attachment id"),
        ("key" = Uuid, Query, description = "Attachment capability key")
    ),
    responses(
        (status = 200, description = "Complete attachment"),
        (status = 206, description = "Requested byte range"),
        (status = 404, description = "Attachment or key not found"),
        (status = 416, description = "Invalid byte range")
    )
)]
pub async fn download_attachment(
    State(state): State<SharedState>,
    Path(id): Path<Uuid>,
    Query(access): Query<AttachmentAccess>,
    headers: HeaderMap,
) -> Response<Body> {
    let metadata = match state.attachment_metadata(id, access.key).await {
        Ok(Some(metadata)) => metadata,
        Ok(None) => return empty_response(StatusCode::NOT_FOUND),
        Err(error) => {
            tracing::error!("load attachment metadata failed: {}", error);
            return empty_response(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    let range = match requested_range(&headers, metadata.size_bytes) {
        Ok(range) => range,
        Err(()) => {
            let mut response = empty_response(StatusCode::RANGE_NOT_SATISFIABLE);
            response.headers_mut().insert(
                CONTENT_RANGE,
                HeaderValue::from_str(&format!("bytes */{}", metadata.size_bytes)).unwrap(),
            );
            return response;
        }
    };
    let (start, end) = range.unwrap_or((0, metadata.size_bytes - 1));
    let length = end - start + 1;
    let data = match state.attachment_chunk(id, access.key, start, length).await {
        Ok(Some(data)) => data,
        Ok(None) => return empty_response(StatusCode::NOT_FOUND),
        Err(error) => {
            tracing::error!("load attachment data failed: {}", error);
            return empty_response(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    let status = if range.is_some() {
        StatusCode::PARTIAL_CONTENT
    } else {
        StatusCode::OK
    };
    let mut response = Response::new(Body::from(data));
    *response.status_mut() = status;
    let response_headers = response.headers_mut();
    response_headers.insert(ACCEPT_RANGES, HeaderValue::from_static("bytes"));
    response_headers.insert(
        CACHE_CONTROL,
        HeaderValue::from_static("private, max-age=31536000, immutable"),
    );
    response_headers.insert(
        CONTENT_LENGTH,
        HeaderValue::from_str(&length.to_string()).unwrap(),
    );
    response_headers.insert(
        CONTENT_TYPE,
        HeaderValue::from_str(&metadata.mime_type)
            .unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream")),
    );
    response_headers.insert(
        CONTENT_DISPOSITION,
        HeaderValue::from_static(if previewable_mime(&metadata.mime_type) {
            "inline"
        } else {
            "attachment"
        }),
    );
    response_headers.insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    if range.is_some() {
        response_headers.insert(
            CONTENT_RANGE,
            HeaderValue::from_str(&format!("bytes {start}-{end}/{}", metadata.size_bytes)).unwrap(),
        );
    }
    response
}

async fn authorize_upload(
    state: &SharedState,
    room_id: Uuid,
    headers: &HeaderMap,
) -> Result<(Room, User), StatusCode> {
    let room = state.room(room_id).await.ok_or(StatusCode::NOT_FOUND)?;
    if room.has_password {
        let password = headers
            .get("x-room-password")
            .and_then(|value| value.to_str().ok())
            .ok_or(StatusCode::UNAUTHORIZED)?;
        let mut hasher = Sha256::new();
        hasher.update(password.as_bytes());
        if hex::encode(hasher.finalize()) != room.password_hash {
            return Err(StatusCode::UNAUTHORIZED);
        }
    }
    let token = headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .and_then(|value| value.parse().ok())
        .ok_or(StatusCode::UNAUTHORIZED)?;
    let user = state
        .session_user(token)
        .await
        .map_err(|error| {
            tracing::error!("validate attachment upload session failed: {}", error);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or(StatusCode::UNAUTHORIZED)?;
    Ok((room, user))
}

fn normalize_file_name(value: &str) -> Result<String, StatusCode> {
    let name = value
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or("file")
        .trim()
        .chars()
        .filter(|character| !character.is_control())
        .collect::<String>();
    if name.is_empty() || name.chars().count() > MAX_FILE_NAME_CHARS {
        return Err(StatusCode::BAD_REQUEST);
    }
    Ok(name)
}

fn normalized_mime(supplied: Option<&str>, file_name: &str) -> String {
    supplied
        .filter(|value| {
            !value.is_empty()
                && value.len() <= 127
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"!#$&^_.+-/".contains(&byte))
        })
        .map(str::to_ascii_lowercase)
        .or_else(|| {
            mime_guess::from_path(file_name)
                .first_raw()
                .map(str::to_string)
        })
        .unwrap_or_else(|| "application/octet-stream".to_string())
}

fn previewable_mime(mime_type: &str) -> bool {
    matches!(
        mime_type,
        "image/avif"
            | "image/gif"
            | "image/jpeg"
            | "image/png"
            | "image/webp"
            | "video/mp4"
            | "video/ogg"
            | "video/quicktime"
            | "video/webm"
    )
}

fn requested_range(headers: &HeaderMap, size: i64) -> Result<Option<(i64, i64)>, ()> {
    let Some(value) = headers.get(RANGE) else {
        return Ok(None);
    };
    let value = value.to_str().map_err(|_| ())?;
    let spec = value.strip_prefix("bytes=").ok_or(())?;
    if spec.contains(',') {
        return Err(());
    }
    let (start, end) = spec.split_once('-').ok_or(())?;
    if start.is_empty() {
        let suffix = end.parse::<i64>().map_err(|_| ())?;
        if suffix <= 0 {
            return Err(());
        }
        return Ok(Some(((size - suffix).max(0), size - 1)));
    }
    let start = start.parse::<i64>().map_err(|_| ())?;
    if start < 0 || start >= size {
        return Err(());
    }
    let end = if end.is_empty() {
        size - 1
    } else {
        end.parse::<i64>().map_err(|_| ())?.min(size - 1)
    };
    if end < start {
        return Err(());
    }
    Ok(Some((start, end)))
}

fn empty_response(status: StatusCode) -> Response<Body> {
    Response::builder()
        .status(status)
        .body(Body::empty())
        .unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_complete_open_and_suffix_ranges() {
        let headers = |value: &'static str| {
            let mut headers = HeaderMap::new();
            headers.insert(RANGE, HeaderValue::from_static(value));
            headers
        };
        assert_eq!(requested_range(&headers("bytes=2-5"), 10), Ok(Some((2, 5))));
        assert_eq!(requested_range(&headers("bytes=7-"), 10), Ok(Some((7, 9))));
        assert_eq!(requested_range(&headers("bytes=-3"), 10), Ok(Some((7, 9))));
        assert!(requested_range(&headers("bytes=10-"), 10).is_err());
    }

    #[test]
    fn strips_paths_and_control_characters_from_file_names() {
        assert_eq!(normalize_file_name("../photos/a\n.png").unwrap(), "a.png");
        assert_eq!(
            normalize_file_name("C:\\temp\\movie.mp4").unwrap(),
            "movie.mp4"
        );
    }
}
