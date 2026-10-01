//! TG-508 HTTP surface: the viewer's notification settings (defaults per chat type, per-chat
//! exceptions). The rules live in `exceptions.rs`; these handlers only translate.

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use chrono::Utc;
use serde::Deserialize;
use utoipa::ToSchema;
use uuid::Uuid;

use super::exceptions::{
    ExceptionRow, NotificationDefaults, NotificationException, NotificationExceptionView,
    NotificationScope, NotificationSettings, SOUNDS,
};
use crate::chats::membership_handlers::session_user;
use crate::state::{with_pool, SharedState};

fn valid_sound(sound: &str) -> bool {
    SOUNDS.contains(&sound)
}

#[utoipa::path(get, path = "/api/users/me/notification-settings",
    responses((status = 200, description = "Defaults per chat type and the per-chat exceptions", body = NotificationSettings)))]
pub async fn get_settings(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> Result<Json<NotificationSettings>, StatusCode> {
    let user = session_user(&state, &headers).await?;
    let mut defaults = Vec::new();
    for scope in [
        NotificationScope::Private,
        NotificationScope::Group,
        NotificationScope::Channel,
    ] {
        defaults.push(
            state
                .notification_defaults(user.id, scope)
                .await
                .map_err(internal)?,
        );
    }
    let rows: Vec<ExceptionRow> = with_pool!(state, |pool| {
        sqlx::query_as(
            "SELECT room_id, enabled, preview, sound FROM notification_exceptions \
             WHERE user_id = $1 ORDER BY updated_at DESC",
        )
        .bind(user.id)
        .fetch_all(pool)
        .await
    })
    .map_err(internal)?;
    let mut exceptions = Vec::with_capacity(rows.len());
    for (chat_id, enabled, preview, sound) in rows {
        // Read-time authorization: only chats the viewer can still read are listed.
        if !state
            .can_read_chat(chat_id, user.id)
            .await
            .map_err(internal)?
        {
            continue;
        }
        let chat_title = state
            .chat(chat_id)
            .await
            .map(|chat| chat.title)
            .unwrap_or_default();
        exceptions.push(NotificationExceptionView {
            chat_id,
            chat_title,
            exception: NotificationException {
                enabled,
                preview,
                sound,
            },
        });
    }
    Ok(Json(NotificationSettings {
        defaults,
        exceptions,
    }))
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct DefaultsWrite {
    pub enabled: bool,
    pub preview: bool,
    pub sound: String,
}

#[utoipa::path(put, path = "/api/users/me/notification-settings/defaults/{scope}",
    params(("scope" = String, Path, description = "private | group | channel")),
    request_body = DefaultsWrite,
    responses((status = 200, description = "Saved", body = NotificationDefaults), (status = 400, description = "Unknown scope or sound")))]
pub async fn put_defaults(
    State(state): State<SharedState>,
    Path(scope): Path<String>,
    headers: HeaderMap,
    Json(write): Json<DefaultsWrite>,
) -> Result<Json<NotificationDefaults>, StatusCode> {
    let user = session_user(&state, &headers).await?;
    let scope = match scope.as_str() {
        "private" => NotificationScope::Private,
        "group" => NotificationScope::Group,
        "channel" => NotificationScope::Channel,
        _ => return Err(StatusCode::BAD_REQUEST),
    };
    if !valid_sound(&write.sound) {
        return Err(StatusCode::BAD_REQUEST);
    }
    with_pool!(state, |pool| {
        sqlx::query(
            "INSERT INTO notification_defaults (user_id, scope, enabled, preview, sound) VALUES ($1, $2, $3, $4, $5) \
             ON CONFLICT (user_id, scope) DO UPDATE SET enabled = excluded.enabled, \
             preview = excluded.preview, sound = excluded.sound",
        )
        .bind(user.id)
        .bind(scope.as_str())
        .bind(write.enabled)
        .bind(write.preview)
        .bind(&write.sound)
        .execute(pool)
        .await
        .map(|_| ())
    })
    .map_err(internal)?;
    Ok(Json(NotificationDefaults {
        scope,
        enabled: write.enabled,
        preview: write.preview,
        sound: write.sound,
    }))
}

#[utoipa::path(put, path = "/api/chats/{id}/notification-exception", params(("id" = Uuid, description = "Chat id")),
    request_body = NotificationException,
    responses((status = 200, description = "Saved (all fields null removes the exception)", body = NotificationException),
        (status = 400, description = "Unknown sound"), (status = 404, description = "Not a member")))]
pub async fn put_exception(
    State(state): State<SharedState>,
    Path(room_id): Path<Uuid>,
    headers: HeaderMap,
    Json(exception): Json<NotificationException>,
) -> Result<Json<NotificationException>, StatusCode> {
    let user = session_user(&state, &headers).await?;
    if !state
        .can_read_chat(room_id, user.id)
        .await
        .map_err(internal)?
    {
        return Err(StatusCode::NOT_FOUND);
    }
    if exception
        .sound
        .as_deref()
        .is_some_and(|sound| !valid_sound(sound))
    {
        return Err(StatusCode::BAD_REQUEST);
    }
    let empty = exception == NotificationException::default();
    with_pool!(state, |pool| {
        async {
            if empty {
                sqlx::query("DELETE FROM notification_exceptions WHERE user_id = $1 AND room_id = $2")
                    .bind(user.id)
                    .bind(room_id)
                    .execute(pool)
                    .await?;
            } else {
                sqlx::query(
                    "INSERT INTO notification_exceptions (user_id, room_id, enabled, preview, sound, updated_at) \
                     VALUES ($1, $2, $3, $4, $5, $6) ON CONFLICT (user_id, room_id) DO UPDATE SET \
                     enabled = excluded.enabled, preview = excluded.preview, sound = excluded.sound, \
                     updated_at = excluded.updated_at",
                )
                .bind(user.id)
                .bind(room_id)
                .bind(exception.enabled)
                .bind(exception.preview)
                .bind(&exception.sound)
                .bind(Utc::now())
                .execute(pool)
                .await?;
            }
            Ok::<_, sqlx::Error>(())
        }
        .await
    })
    .map_err(internal)?;
    Ok(Json(exception))
}

fn internal(error: sqlx::Error) -> StatusCode {
    tracing::error!("notification settings: {error}");
    StatusCode::INTERNAL_SERVER_ERROR
}
