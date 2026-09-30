//! Cloud drafts (TG-008): one composer draft per (chat, account), synced across devices.
//!
//! The wire shapes are frozen in `docs/devlog/TG-008.md`. Writes are idempotent — a PUT
//! identical to the stored draft changes nothing and broadcasts nothing, so client retries
//! and debounce races cannot spam an account's other devices. Every real change is announced
//! through `AppState::broadcast`, and only there: the `draft_updated` privacy guarantee
//! (delivery to the drafting account's own connections exclusively) lives in the transport
//! filter `frame_visible_to` (`src/realtime/protocol.rs`), which a direct socket send would
//! bypass (`docs/devlog/TG-007.md`, Residual risk).

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::models::{ChatMessage, User};
use crate::state::{with_pool, AppState, SharedState};
use crate::user_handlers::bearer_token;

/// Mirrors `MAX_MESSAGE_CHARS` (`src/realtime/auth.rs`): a draft longer than the message cap
/// could never be sent, so it is refused instead of silently truncated — truncation would
/// leave the account's devices disagreeing about the draft's tail.
const MAX_DRAFT_CHARS: usize = 4096;

/// A stored draft, serialised exactly like the frozen `draft_updated` frame payload
/// (`docs/devlog/TG-007.md` §3) so clients parse one shape on both transports.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ChatDraft {
    pub user_id: Uuid,
    pub text: String,
    pub reply_to_message_id: Option<Uuid>,
    pub topic_id: Option<Uuid>,
    pub updated_at: DateTime<Utc>,
}

/// `PUT /api/chats/:id/draft` request body. `updated_at` is deliberately absent: device
/// clocks skew, so the server stamps every write.
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct DraftWrite {
    pub text: String,
    #[serde(default)]
    pub reply_to_message_id: Option<Uuid>,
    #[serde(default)]
    pub topic_id: Option<Uuid>,
}

impl DraftWrite {
    /// An empty text with no reply target is Telegram's "clear the draft".
    /// An empty text WITH a reply is real composer state: a picked reply target, no text yet.
    fn clears(&self) -> bool {
        self.text.is_empty() && self.reply_to_message_id.is_none()
    }

    fn matches(&self, stored: &ChatDraft) -> bool {
        self.text == stored.text
            && self.reply_to_message_id == stored.reply_to_message_id
            && self.topic_id == stored.topic_id
    }
}

/// What a save did, so the handler broadcasts only real changes.
pub(crate) struct DraftSave {
    pub draft: ChatDraft,
    pub changed: bool,
}

/// `(text, reply_to_message_id, topic_id, updated_at)` as selected from `chat_drafts`.
type DraftRow = (String, Option<Uuid>, Option<Uuid>, DateTime<Utc>);

impl AppState {
    pub async fn chat_draft(
        &self,
        room_id: Uuid,
        user_id: Uuid,
    ) -> Result<Option<ChatDraft>, sqlx::Error> {
        let row: Option<DraftRow> = with_pool!(self, |pool| {
            sqlx::query_as(
                "SELECT text, reply_to_message_id, topic_id, updated_at \
                     FROM chat_drafts WHERE room_id = $1 AND user_id = $2",
            )
            .bind(room_id)
            .bind(user_id)
            .fetch_optional(pool)
            .await
        })?;
        Ok(row.map(
            |(text, reply_to_message_id, topic_id, updated_at)| ChatDraft {
                user_id,
                text,
                reply_to_message_id,
                topic_id,
                updated_at,
            },
        ))
    }

    /// Whether `message_id` is a non-recalled message of this chat — the only reply target a
    /// draft may point at. Cross-chat pointers would leak message existence across the
    /// authorization boundary.
    async fn draft_reply_target_exists(
        &self,
        room_id: Uuid,
        message_id: Uuid,
    ) -> Result<bool, sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM messages \
                 WHERE id = $1 AND room_id = $2 AND recalled_at IS NULL)",
            )
            .bind(message_id)
            .bind(room_id)
            .fetch_one(pool)
            .await
        })
    }

    /// Idempotent save: an identical PUT returns the stored row untouched (original
    /// `updated_at`, `changed: false`); a clear of an absent draft is a no-op. The write
    /// itself is an atomic UPSERT so racing non-identical PUTs cannot error — last writer
    /// wins and each broadcasts its own state.
    pub(crate) async fn save_chat_draft(
        &self,
        room_id: Uuid,
        user_id: Uuid,
        write: &DraftWrite,
    ) -> Result<DraftSave, sqlx::Error> {
        let stored = self.chat_draft(room_id, user_id).await?;
        if write.clears() {
            let changed = stored.is_some();
            if changed {
                with_pool!(self, |pool| {
                    sqlx::query("DELETE FROM chat_drafts WHERE room_id = $1 AND user_id = $2")
                        .bind(room_id)
                        .bind(user_id)
                        .execute(pool)
                        .await
                        .map(|_| ())
                })?;
            }
            return Ok(DraftSave {
                draft: ChatDraft {
                    user_id,
                    text: String::new(),
                    reply_to_message_id: None,
                    topic_id: None,
                    updated_at: Utc::now(),
                },
                changed,
            });
        }
        if let Some(stored) = stored {
            if write.matches(&stored) {
                return Ok(DraftSave {
                    draft: stored,
                    changed: false,
                });
            }
        }
        let updated_at = Utc::now();
        with_pool!(self, |pool| {
            sqlx::query(
                "INSERT INTO chat_drafts \
                 (room_id, user_id, text, reply_to_message_id, topic_id, updated_at) \
                 VALUES ($1, $2, $3, $4, $5, $6) \
                 ON CONFLICT (room_id, user_id) DO UPDATE SET \
                   text = excluded.text, \
                   reply_to_message_id = excluded.reply_to_message_id, \
                   topic_id = excluded.topic_id, \
                   updated_at = excluded.updated_at",
            )
            .bind(room_id)
            .bind(user_id)
            .bind(&write.text)
            .bind(write.reply_to_message_id)
            .bind(write.topic_id)
            .bind(updated_at)
            .execute(pool)
            .await
            .map(|_| ())
        })?;
        Ok(DraftSave {
            draft: ChatDraft {
                user_id,
                text: write.text.clone(),
                reply_to_message_id: write.reply_to_message_id,
                topic_id: write.topic_id,
                updated_at,
            },
            changed: true,
        })
    }
}

/// 404 for an unknown or deleted chat, then the caller's session, then active membership.
/// A draft is the member's own composer state: no permission key applies — the role registry
/// and the chat-type intrinsic layer govern *sending*, not composing.
async fn draft_actor(
    state: &SharedState,
    room_id: Uuid,
    headers: &HeaderMap,
) -> Result<User, StatusCode> {
    state.chat(room_id).await.ok_or(StatusCode::NOT_FOUND)?;
    let user = state
        .session_user(bearer_token(headers)?)
        .await
        .map_err(internal_error)?
        .ok_or(StatusCode::UNAUTHORIZED)?;
    state
        .membership_identity(room_id, user.id)
        .await
        .map_err(internal_error)?
        .is_some_and(|(status, _)| status == "active")
        .then_some(user)
        .ok_or(StatusCode::FORBIDDEN)
}

#[utoipa::path(
    get,
    path = "/api/chats/{id}/draft",
    params(("id" = Uuid, Path, description = "Chat identifier")),
    responses(
        (status = 200, description = "The caller's stored draft, or JSON null when none", body = Option<ChatDraft>),
        (status = 401, description = "Missing session"),
        (status = 403, description = "Account is not an active chat member"),
        (status = 404, description = "Chat not found")
    )
)]
pub async fn get_draft(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(room_id): Path<Uuid>,
) -> Result<Json<Option<ChatDraft>>, StatusCode> {
    let user = draft_actor(&state, room_id, &headers).await?;
    state
        .chat_draft(room_id, user.id)
        .await
        .map(Json)
        .map_err(internal_error)
}

#[utoipa::path(
    put,
    path = "/api/chats/{id}/draft",
    params(("id" = Uuid, Path, description = "Chat identifier")),
    request_body = DraftWrite,
    responses(
        (status = 200, description = "The stored draft after the (idempotent) write", body = ChatDraft),
        (status = 400, description = "Draft exceeds 4096 characters, or the reply target is not a live message of this chat"),
        (status = 401, description = "Missing session"),
        (status = 403, description = "Account is not an active chat member"),
        (status = 404, description = "Chat not found")
    )
)]
pub async fn put_draft(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(room_id): Path<Uuid>,
    Json(write): Json<DraftWrite>,
) -> Result<Json<ChatDraft>, StatusCode> {
    let user = draft_actor(&state, room_id, &headers).await?;
    if write.text.chars().count() > MAX_DRAFT_CHARS {
        return Err(StatusCode::BAD_REQUEST);
    }
    if let Some(reply_id) = write.reply_to_message_id {
        if !state
            .draft_reply_target_exists(room_id, reply_id)
            .await
            .map_err(internal_error)?
        {
            return Err(StatusCode::BAD_REQUEST);
        }
    }
    let saved = state
        .save_chat_draft(room_id, user.id, &write)
        .await
        .map_err(internal_error)?;
    if saved.changed {
        state
            .broadcast(
                room_id,
                ChatMessage::DraftUpdated {
                    user_id: saved.draft.user_id,
                    text: saved.draft.text.clone(),
                    reply_to_message_id: saved.draft.reply_to_message_id,
                    topic_id: saved.draft.topic_id,
                    updated_at: saved.draft.updated_at,
                },
            )
            .await;
    }
    Ok(Json(saved.draft))
}

fn internal_error(error: sqlx::Error) -> StatusCode {
    tracing::error!("chat draft operation failed: {error}");
    StatusCode::INTERNAL_SERVER_ERROR
}
