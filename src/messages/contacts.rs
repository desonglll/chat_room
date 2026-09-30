//! TG-410: contact cards and message translation.
//!
//! A contact card is an ordinary message (`media_kind = 'contact'`) with a `message_contacts`
//! row holding a snapshot of the shared account; it goes through the normal post gate (topic,
//! slow mode) and `message.send`, and is broadcast like any message. Translation is a
//! projection for one viewer through the configured AI provider; when no provider is
//! configured the endpoint reports it and clients hide the entry instead of showing an error.

use std::collections::HashMap;

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::QueryBuilder;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::chats::membership_handlers::session_user;
use crate::models::StoredMessage;
use crate::realtime::protocol::stored_message_to_chat;
use crate::state::{with_pool, AppState, SharedState};

pub const MEDIA_KIND_CONTACT: &str = "contact";

/// The shared account as the card shows it (a snapshot taken when it was sent).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ContactCard {
    /// `None` once the shared account was deleted.
    pub user_id: Option<Uuid>,
    pub username: String,
    pub display_name: String,
    pub avatar_emoji: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct SendContactRequest {
    pub user_id: Uuid,
    #[serde(default)]
    pub reply_to: Option<Uuid>,
    /// TG-204: the forum topic; absent = General.
    #[serde(default)]
    pub topic_id: Option<Uuid>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct TranslateRequest {
    /// A language name or tag the model understands, e.g. `中文`, `English`, `ja`.
    pub target_language: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TranslateResponse {
    pub message_id: Uuid,
    pub target_language: String,
    pub text: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TranslationAvailability {
    /// False when no AI provider is configured: clients hide the «翻译» entry.
    pub available: bool,
}

impl AppState {
    /// Attach contact cards to loaded messages (every loader runs this, like TG-401 voice).
    pub(crate) async fn attach_message_contacts(
        &self,
        messages: &mut [StoredMessage],
    ) -> Result<(), sqlx::Error> {
        // A card has no attachment, so TG-302's media step never set `media_kind` on it; look
        // every loaded non-attachment message up here and set both fields.
        let ids: Vec<Uuid> = messages
            .iter()
            .filter(|message| message.attachment.is_none())
            .map(|message| message.id)
            .collect();
        if ids.is_empty() {
            return Ok(());
        }
        let rows: Vec<(Uuid, Option<Uuid>, String, String, String)> = with_pool!(self, |pool| {
            let mut query = QueryBuilder::new(
                "SELECT message_id, user_id, username, display_name, avatar_emoji \
                 FROM message_contacts WHERE message_id IN (",
            );
            {
                let mut values = query.separated(", ");
                for id in &ids {
                    values.push_bind(*id);
                }
            }
            query.push(")");
            query.build_query_as().fetch_all(pool).await
        })?;
        let mut cards: HashMap<Uuid, ContactCard> = rows
            .into_iter()
            .map(|(id, user_id, username, display_name, avatar_emoji)| {
                (
                    id,
                    ContactCard {
                        user_id,
                        username,
                        display_name,
                        avatar_emoji,
                    },
                )
            })
            .collect();
        for message in messages.iter_mut() {
            if let Some(card) = cards.remove(&message.id) {
                message.media_kind = Some(MEDIA_KIND_CONTACT.to_string());
                if message.recalled_at.is_none() {
                    message.contact = Some(card);
                }
            }
        }
        Ok(())
    }

    async fn insert_contact_message(
        &self,
        room_id: Uuid,
        sender_id: Uuid,
        sender_name: &str,
        card: &ContactCard,
        reply_to: Option<Uuid>,
        topic_id: Option<Uuid>,
    ) -> Result<Option<Uuid>, sqlx::Error> {
        let message_id = Uuid::new_v4();
        let created_at = Utc::now();
        let reply_to = self
            .reply_preview(room_id, reply_to)
            .await?
            .map(|reply| reply.message_id);
        with_pool!(self, |pool| {
            async {
                let mut tx = pool.begin().await?;
                let inserted = sqlx::query(
                    "INSERT INTO messages (id, room_id, sender_id, sender, content, reply_to_id, \
                     media_kind, created_at, topic_id) \
                     SELECT $1, $2, $3, $4, '', $5, $6, $7, $8 \
                     WHERE EXISTS (SELECT 1 FROM chat_members WHERE chat_members.room_id = $2 \
                       AND chat_members.user_id = $3 AND chat_members.status = 'active')",
                )
                .bind(message_id)
                .bind(room_id)
                .bind(sender_id)
                .bind(sender_name)
                .bind(reply_to)
                .bind(MEDIA_KIND_CONTACT)
                .bind(created_at)
                .bind(topic_id)
                .execute(&mut *tx)
                .await?
                .rows_affected()
                    > 0;
                if !inserted {
                    return Ok::<_, sqlx::Error>(None);
                }
                sqlx::query(
                    "INSERT INTO message_contacts (message_id, user_id, username, display_name, avatar_emoji) \
                     VALUES ($1, $2, $3, $4, $5)",
                )
                .bind(message_id)
                .bind(card.user_id)
                .bind(&card.username)
                .bind(&card.display_name)
                .bind(&card.avatar_emoji)
                .execute(&mut *tx)
                .await?;
                tx.commit().await?;
                Ok(Some(message_id))
            }
            .await
        })
    }
}

#[utoipa::path(post, path = "/api/chats/{id}/contact-messages", params(("id" = Uuid, description = "Chat id")),
    request_body = SendContactRequest,
    responses((status = 201, description = "The contact card message (also broadcast)", body = StoredMessage),
        (status = 403, description = "May not send here (permission, closed topic, slow mode)"),
        (status = 404, description = "No such chat or account")))]
pub async fn send_contact(
    State(state): State<SharedState>,
    Path(room_id): Path<Uuid>,
    headers: HeaderMap,
    Json(request): Json<SendContactRequest>,
) -> Result<(StatusCode, Json<StoredMessage>), StatusCode> {
    let sender = session_user(&state, &headers).await?;
    if !state
        .has_chat_permission(room_id, sender.id, "message.send")
        .await
        .map_err(internal)?
    {
        return Err(StatusCode::FORBIDDEN);
    }
    let topic_id = state
        .resolve_post_topic(room_id, sender.id, request.topic_id)
        .await
        .map_err(StatusCode::from)?;
    let shared = state
        .user_by_id(request.user_id)
        .await
        .map_err(internal)?
        .ok_or(StatusCode::NOT_FOUND)?;
    let card = ContactCard {
        user_id: Some(shared.id),
        username: shared.username,
        display_name: shared.display_name,
        avatar_emoji: shared.avatar_emoji,
    };
    let display_name = state.resolve_display_name(room_id, &sender).await;
    let message_id = state
        .insert_contact_message(
            room_id,
            sender.id,
            &display_name,
            &card,
            request.reply_to,
            topic_id,
        )
        .await
        .map_err(internal)?
        .ok_or(StatusCode::FORBIDDEN)?;
    state.invalidate_message_cache(room_id).await;
    let message = state
        .message_by_id(message_id, Some(sender.id))
        .await
        .map_err(internal)?
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
    state
        .broadcast(room_id, stored_message_to_chat(message.clone()))
        .await;
    Ok((StatusCode::CREATED, Json(message)))
}

#[utoipa::path(get, path = "/api/translation",
    responses((status = 200, description = "Whether «翻译» is available", body = TranslationAvailability)))]
pub async fn translation_availability(
    State(state): State<SharedState>,
) -> Json<TranslationAvailability> {
    Json(TranslationAvailability {
        available: state.ai_enabled(),
    })
}

#[utoipa::path(post, path = "/api/messages/{id}/translate", params(("id" = Uuid, description = "Message id")),
    request_body = TranslateRequest,
    responses((status = 200, description = "The translation (never stored over the original)", body = TranslateResponse),
        (status = 404, description = "No such message, or not readable by the caller"),
        (status = 503, description = "No AI provider is configured")))]
pub async fn translate_message(
    State(state): State<SharedState>,
    Path(message_id): Path<Uuid>,
    headers: HeaderMap,
    Json(request): Json<TranslateRequest>,
) -> Result<Json<TranslateResponse>, StatusCode> {
    let user = session_user(&state, &headers).await?;
    let Some(assistant) = state.ai_assistant.as_ref() else {
        return Err(StatusCode::SERVICE_UNAVAILABLE);
    };
    let target_language = request.target_language.trim();
    if target_language.is_empty() || target_language.chars().count() > 32 {
        return Err(StatusCode::BAD_REQUEST);
    }
    let room_id = state
        .message_room_id(message_id)
        .await
        .map_err(internal)?
        .ok_or(StatusCode::NOT_FOUND)?;
    // Read-time authorization, like every other read of a message.
    if !state
        .can_read_chat(room_id, user.id)
        .await
        .map_err(internal)?
    {
        return Err(StatusCode::NOT_FOUND);
    }
    let message = state
        .message_by_id(message_id, Some(user.id))
        .await
        .map_err(internal)?
        .filter(|message| message.recalled_at.is_none() && !message.content.trim().is_empty())
        .ok_or(StatusCode::NOT_FOUND)?;
    let text = assistant
        .translate(&message.content, target_language)
        .await
        .map_err(|error| {
            tracing::warn!("translation failed: {error:#}");
            StatusCode::BAD_GATEWAY
        })?;
    Ok(Json(TranslateResponse {
        message_id,
        target_language: target_language.to_string(),
        text,
    }))
}

fn internal(error: sqlx::Error) -> StatusCode {
    tracing::error!("contacts/translation: {error}");
    StatusCode::INTERNAL_SERVER_ERROR
}
