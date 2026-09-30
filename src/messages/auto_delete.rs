//! TG-405 auto-delete ("自毁计时器"): a chat-wide timer after which messages are deleted for
//! everyone. Setting the timer only stamps messages sent afterwards (`messages.auto_delete_at`,
//! written by a database trigger on every insert path); a background sweeper deletes expired
//! messages in batches, recomputes their attachments' orphan state (a forwarded copy or a
//! favorite may still hold the file) and tells clients with a `messages_deleted` frame.
//!
//! The sweep is idempotent and resumable: it only ever acts on rows that are still present
//! and past their deadline, so a crash mid-batch is finished by the next tick.

use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::models::{Chat, ChatMessage};
use crate::state::{with_pool, AppState, SharedState};

/// Telegram's choices: off, 1 day, 1 week, 1 month.
pub const AUTO_DELETE_CHOICES: [i64; 4] = [0, 86_400, 604_800, 2_592_000];
/// How often the sweeper looks for expired messages.
const SWEEP_INTERVAL: Duration = Duration::from_secs(10);
/// Messages deleted per sweep transaction.
const SWEEP_BATCH: i64 = 500;

#[derive(Debug, Deserialize, ToSchema)]
pub struct AutoDeleteRequest {
    /// One of 0 (off), 86400, 604800, 2592000.
    pub seconds: i64,
}

/// «1 天», «1 周», «1 个月» — also used in the system notice.
pub fn auto_delete_label(seconds: i64) -> &'static str {
    match seconds {
        86_400 => "1 天",
        604_800 => "1 周",
        2_592_000 => "1 个月",
        _ => "关闭",
    }
}

impl AppState {
    /// Set the chat's timer. Answers the updated chat.
    pub async fn set_auto_delete(
        &self,
        room_id: Uuid,
        seconds: i64,
    ) -> Result<Option<Chat>, sqlx::Error> {
        let updated = with_pool!(self, |pool| {
            sqlx::query(
                "UPDATE chats SET auto_delete_seconds = $1 WHERE id = $2 AND deleted_at IS NULL",
            )
            .bind(seconds)
            .bind(room_id)
            .execute(pool)
            .await
            .map(|result| result.rows_affected())
        })?;
        if updated == 0 {
            return Ok(None);
        }
        let Some(mut chat) = self.chat(room_id).await else {
            return Ok(None);
        };
        chat.auto_delete_seconds = seconds;
        self.cache_updated_chat(chat.clone()).await;
        Ok(Some(chat))
    }

    /// Delete up to one batch of expired messages. Answers how many were deleted.
    pub async fn sweep_auto_deleted_messages(
        &self,
        now: DateTime<Utc>,
    ) -> Result<usize, sqlx::Error> {
        let deleted: Vec<(Uuid, Uuid, Option<Uuid>)> = with_pool!(self, |pool| {
            async {
                let mut transaction = pool.begin().await?;
                let expired: Vec<Uuid> = sqlx::query_scalar(
                    "SELECT id FROM messages WHERE auto_delete_at IS NOT NULL AND auto_delete_at <= $1 \
                     ORDER BY auto_delete_at LIMIT $2",
                )
                .bind(now)
                .bind(SWEEP_BATCH)
                .fetch_all(&mut *transaction)
                .await?;
                let mut deleted = Vec::with_capacity(expired.len());
                for id in expired {
                    // Dependent rows (reactions, pins, mentions, polls, entities, voice notes,
                    // notifications, reply links) follow their `ON DELETE` rules, as for a
                    // deleted forum topic (TG-204).
                    let row: Option<(Uuid, Uuid, Option<Uuid>)> = sqlx::query_as(
                        "DELETE FROM messages WHERE id = $1 RETURNING id, room_id, attachment_id",
                    )
                    .bind(id)
                    .fetch_optional(&mut *transaction)
                    .await?;
                    deleted.extend(row);
                }
                transaction.commit().await?;
                Ok::<_, sqlx::Error>(deleted)
            }
            .await
        })?;
        if deleted.is_empty() {
            return Ok(0);
        }
        let mut attachments: Vec<Uuid> = deleted
            .iter()
            .filter_map(|(_, _, attachment)| *attachment)
            .collect();
        attachments.sort();
        attachments.dedup();
        for attachment_id in attachments {
            if let Err(error) = self.recompute_attachment_orphan_status(attachment_id).await {
                tracing::warn!("auto-delete: recompute attachment orphan status failed: {error:#}");
            }
        }
        let mut by_room: Vec<(Uuid, Vec<Uuid>)> = Vec::new();
        for (message_id, room_id, _) in &deleted {
            match by_room.iter_mut().find(|(room, _)| room == room_id) {
                Some((_, ids)) => ids.push(*message_id),
                None => by_room.push((*room_id, vec![*message_id])),
            }
        }
        for (room_id, message_ids) in by_room {
            self.invalidate_message_cache(room_id).await;
            self.broadcast(room_id, ChatMessage::MessagesDeleted { message_ids })
                .await;
        }
        Ok(deleted.len())
    }
}

static SWEEPERS: Mutex<Vec<Weak<AppState>>> = Mutex::new(Vec::new());

/// Start the auto-delete sweeper for this state once. Called when the router is built.
pub fn ensure_auto_delete_sweeper(state: Arc<AppState>) {
    {
        let mut sweepers = SWEEPERS
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        sweepers.retain(|existing| existing.strong_count() > 0);
        if sweepers
            .iter()
            .any(|existing| std::ptr::eq(existing.as_ptr(), Arc::as_ptr(&state)))
        {
            return;
        }
        sweepers.push(Arc::downgrade(&state));
    }
    let weak = Arc::downgrade(&state);
    drop(state);
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(SWEEP_INTERVAL).await;
            let Some(state) = weak.upgrade() else {
                return;
            };
            // Drain whole batches back to back; wait a tick only when caught up.
            loop {
                match state.sweep_auto_deleted_messages(Utc::now()).await {
                    Ok(count) if count as i64 >= SWEEP_BATCH => continue,
                    Ok(_) => break,
                    Err(error) => {
                        tracing::warn!("auto-delete sweep failed: {error}");
                        break;
                    }
                }
            }
        }
    });
}

#[utoipa::path(put, path = "/api/chats/{id}/auto-delete", params(("id" = Uuid, description = "Chat id")),
    request_body = AutoDeleteRequest,
    responses((status = 200, description = "Timer set; applies to messages sent from now on", body = Chat),
        (status = 400, description = "Not one of the allowed timers"),
        (status = 403, description = "Missing chat.info (either participant may set it in a private chat)"),
        (status = 404, description = "No such chat, or not a member")))]
pub async fn put_auto_delete(
    State(state): State<SharedState>,
    Path(room_id): Path<Uuid>,
    headers: HeaderMap,
    Json(request): Json<AutoDeleteRequest>,
) -> Result<Json<Chat>, StatusCode> {
    if !AUTO_DELETE_CHOICES.contains(&request.seconds) {
        return Err(StatusCode::BAD_REQUEST);
    }
    let user = crate::chats::membership_handlers::session_user(&state, &headers).await?;
    let chat = state.chat(room_id).await.ok_or(StatusCode::NOT_FOUND)?;
    if !state
        .can_read_chat(room_id, user.id)
        .await
        .map_err(internal)?
    {
        return Err(StatusCode::NOT_FOUND);
    }
    // Telegram: either side of a private chat may set it; in groups and channels it is an
    // admin right (changing the chat's info).
    let allowed = chat.chat_type == crate::chats::chat_type::ChatType::Private
        || state
            .has_chat_permission(room_id, user.id, "chat.info")
            .await
            .map_err(internal)?;
    if !allowed {
        return Err(StatusCode::FORBIDDEN);
    }
    let updated = state
        .set_auto_delete(room_id, request.seconds)
        .await
        .map_err(internal)?
        .ok_or(StatusCode::NOT_FOUND)?;
    let who = state.resolve_display_name(room_id, &user).await;
    let content = if request.seconds > 0 {
        format!(
            "{who} 将自动删除时间设为 {}",
            auto_delete_label(request.seconds)
        )
    } else {
        format!("{who} 关闭了自动删除")
    };
    let mut announced = updated.clone();
    announced.membership_status = None;
    announced.membership_role = None;
    state
        .broadcast(room_id, ChatMessage::ChatUpdated { chat: announced })
        .await;
    state
        .broadcast(
            room_id,
            ChatMessage::System {
                content,
                members: None,
                participants: None,
            },
        )
        .await;
    Ok(Json(updated))
}

fn internal(error: sqlx::Error) -> StatusCode {
    tracing::error!("auto-delete: {error}");
    StatusCode::INTERNAL_SERVER_ERROR
}
