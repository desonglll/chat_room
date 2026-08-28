//! Authorized Room context preparation for durable AI analysis.

use std::collections::HashSet;

use axum::http::StatusCode;
use uuid::Uuid;

use crate::ai::{bounded_conversation_context_to_toon, AiContextMessage};
use crate::ai_threads::{AiCitationAttachment, AiCitationSource};
use crate::handlers::authorize_room;
use crate::messages::store::MessageCursor;
use crate::models::{Room, StoredMessage};
use crate::state::SharedState;

const MAX_CONTEXT_MESSAGE_CHARS: usize = 1_500;
const MAX_CONTEXT_TOON_BYTES: usize = 256 * 1024;
const MESSAGE_HISTORY_PAGE_SIZE: usize = 500;

pub(crate) struct PreparedRoomContext {
    pub toon_context: String,
    pub context_message_count: usize,
    pub message_ids: HashSet<Uuid>,
    pub sources: Vec<AiCitationSource>,
}

pub(crate) async fn room_context_for_user(
    state: &SharedState,
    user_id: Uuid,
    room_id: Uuid,
    headers: &axum::http::HeaderMap,
) -> Result<PreparedRoomContext, StatusCode> {
    let room = state.room(room_id).await.ok_or(StatusCode::NOT_FOUND)?;
    state
        .conversation_summary(user_id, room_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::FORBIDDEN)?;
    require_room_password(&room, headers)?;
    room_context_for_authorized_user(state, user_id, room_id).await
}

pub(crate) async fn room_context_for_authorized_user(
    state: &SharedState,
    user_id: Uuid,
    room_id: Uuid,
) -> Result<PreparedRoomContext, StatusCode> {
    room_context_for_authorized_user_with_limit(
        state,
        user_id,
        room_id,
        state.ai_analysis_context_messages(),
    )
    .await
}

pub(crate) async fn room_context_for_authorized_user_with_limit(
    state: &SharedState,
    user_id: Uuid,
    room_id: Uuid,
    limit: usize,
) -> Result<PreparedRoomContext, StatusCode> {
    let room = state.room(room_id).await.ok_or(StatusCode::NOT_FOUND)?;
    let conversation = state
        .conversation_summary(user_id, room_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::FORBIDDEN)?;
    let history = load_room_history(state, room.id, user_id, limit).await?;
    let mut sources = Vec::new();
    let mut context = Vec::new();
    for message in history
        .into_iter()
        .filter(|message| message.recalled_at.is_none())
    {
        let source = message
            .attachment
            .as_ref()
            .map_or_else(String::new, |attachment| {
                let label = format!("A{}", sources.len() + 1);
                sources.push(AiCitationSource {
                    label: label.clone(),
                    room_id,
                    message_id: message.id,
                    sender: message.sender.clone(),
                    sent_at: message.created_at,
                    excerpt: if message.content.trim().is_empty() {
                        attachment.file_name.clone()
                    } else {
                        truncate_chars(&message.content, 280)
                    },
                    score: None,
                    score_kind: "attachment".into(),
                    attachment: Some(AiCitationAttachment {
                        id: attachment.id,
                        file_name: attachment.file_name.clone(),
                        mime_type: attachment.mime_type.clone(),
                        size_bytes: attachment.size_bytes,
                        download_url: attachment.download_url.clone(),
                        is_sensitive: attachment.is_sensitive,
                    }),
                });
                label
            });
        let attachment = message.attachment.map_or_else(String::new, |attachment| {
            format!(
                "{} ({}, {} bytes)",
                attachment.file_name, attachment.mime_type, attachment.size_bytes
            )
        });
        context.push(AiContextMessage {
            message_id: message.id.to_string(),
            sent_at: message.created_at.to_rfc3339(),
            sender: message.sender,
            content: truncate_chars(&message.content, MAX_CONTEXT_MESSAGE_CHARS),
            source,
            attachment,
        });
    }
    let toon_context = bounded_conversation_context_to_toon(
        &conversation.title,
        &mut context,
        MAX_CONTEXT_TOON_BYTES,
    )
    .map_err(|error| {
        tracing::error!(%room_id, "encode AI analysis context failed: {error}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let active_sources: HashSet<&str> = context
        .iter()
        .map(|message| message.source.as_str())
        .filter(|source| !source.is_empty())
        .collect();
    sources.retain(|source| active_sources.contains(source.label.as_str()));
    let message_ids = context
        .iter()
        .filter_map(|message| Uuid::parse_str(&message.message_id).ok())
        .collect();
    Ok(PreparedRoomContext {
        toon_context,
        context_message_count: context.len(),
        message_ids,
        sources,
    })
}

async fn load_room_history(
    state: &SharedState,
    room_id: Uuid,
    user_id: Uuid,
    limit: usize,
) -> Result<Vec<StoredMessage>, StatusCode> {
    let mut pages = Vec::new();
    let mut remaining = limit;
    let mut through: Option<MessageCursor> = None;
    while remaining > 0 {
        let overlap = usize::from(through.is_some());
        let requested = remaining
            .saturating_add(overlap)
            .min(MESSAGE_HISTORY_PAGE_SIZE);
        let mut page = state
            .message_history(room_id, requested as i64, through.as_ref(), Some(user_id))
            .await
            .map_err(|error| {
                tracing::error!(%room_id, "load AI analysis context failed: {error}");
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
        let fetched = page.len();
        if let Some(cursor) = &through {
            page.retain(|message| message.id != cursor.id);
        }
        if page.is_empty() {
            break;
        }
        remaining = remaining.saturating_sub(page.len());
        through = page.first().map(|message| MessageCursor {
            created_at: message.created_at,
            id: message.id,
        });
        pages.push(page);
        if fetched < requested {
            break;
        }
    }
    pages.reverse();
    Ok(pages.into_iter().flatten().collect())
}

pub(crate) fn require_room_password(
    room: &Room,
    headers: &axum::http::HeaderMap,
) -> Result<(), StatusCode> {
    if !room.has_password {
        return Ok(());
    }
    let supplied = headers
        .get("x-room-password")
        .and_then(|value| value.to_str().ok());
    authorize_room(room, supplied)
        .then_some(())
        .ok_or(StatusCode::UNAUTHORIZED)
}

fn truncate_chars(value: &str, limit: usize) -> String {
    if value.chars().count() <= limit {
        return value.to_owned();
    }
    let mut truncated: String = value.chars().take(limit.saturating_sub(1)).collect();
    truncated.push('…');
    truncated
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use chrono::{Duration, Utc};
    use uuid::Uuid;

    use super::room_context_for_authorized_user_with_limit;
    use crate::{models::Room, state::AppState};

    #[tokio::test]
    async fn full_room_context_reads_across_message_history_pages() {
        let state = Arc::new(AppState::new().await.unwrap());
        let owner = state
            .insert_user("ai-context-owner", "unused")
            .await
            .unwrap();
        let now = Utc::now();
        let room = Room {
            id: Uuid::new_v4(),
            name: "Long room".into(),
            password_hash: String::new(),
            has_password: false,
            creator_user_id: Some(owner.id),
            join_policy: "open".into(),
            avatar_emoji: String::new(),
            description: String::new(),
            membership_status: None,
            membership_role: None,
            unread_count: 0,
            created_at: now,
        };
        state
            .create_room_with_owner(room.clone(), owner.id)
            .await
            .unwrap();
        let mut transaction = state.pool().begin().await.unwrap();
        for index in 0..501 {
            sqlx::query(
                "INSERT INTO messages (id, room_id, sender_id, sender, content, created_at) \
                 VALUES (?, ?, ?, 'owner', ?, ?)",
            )
            .bind(Uuid::new_v4())
            .bind(room.id)
            .bind(owner.id)
            .bind(format!("history-message-{index}"))
            .bind(now + Duration::milliseconds(index))
            .execute(&mut *transaction)
            .await
            .unwrap();
        }
        transaction.commit().await.unwrap();

        let context = room_context_for_authorized_user_with_limit(&state, owner.id, room.id, 501)
            .await
            .unwrap();

        assert_eq!(context.context_message_count, 501);
        assert!(context.toon_context.contains("history-message-0"));
        assert!(context.toon_context.contains("history-message-500"));
    }
}
