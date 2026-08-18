//! Mutations triggered by authenticated client WebSocket frames.

use uuid::Uuid;

use crate::models::{ChatMessage, RoomMember, User};
use crate::state::SharedState;
use crate::ws_auth::{normalize_message, normalize_typing};

/// Match `@username` tokens in `content` against the room's active participants.
/// A match requires a non-alphanumeric (or end-of-string) boundary right after the
/// username so `@bob` doesn't spuriously match a message that says `@bobby`.
fn extract_mentions(content: &str, participants: &[RoomMember], exclude: Uuid) -> Vec<Uuid> {
    participants
        .iter()
        .filter(|member| member.user_id != exclude && !member.username.is_empty())
        .filter(|member| {
            let needle = format!("@{}", member.username);
            content.match_indices(&needle).any(|(start, matched)| {
                content[start + matched.len()..]
                    .chars()
                    .next()
                    .is_none_or(|ch| !ch.is_alphanumeric() && ch != '_')
            })
        })
        .map(|member| member.user_id)
        .collect()
}

pub async fn handle_client_message(
    state: &SharedState,
    room_id: Uuid,
    user: &User,
    message: ChatMessage,
) {
    match message {
        ChatMessage::Message { content, reply_to } => {
            let Some(content) = normalize_message(content) else {
                tracing::warn!("ignored invalid message from {}", user.username);
                return;
            };
            let display_name = state.resolve_display_name(room_id, user).await;
            state
                .broadcast(
                    room_id,
                    ChatMessage::Typing {
                        content: String::new(),
                        user_id: Some(user.id),
                        username: Some(display_name.clone()),
                    },
                )
                .await;
            match state
                .store_message(
                    room_id,
                    user.id,
                    &display_name,
                    &user.avatar_emoji,
                    &content,
                    reply_to,
                )
                .await
            {
                Ok(stored) => {
                    let participants = state.room_participants(room_id).await.unwrap_or_default();
                    let mentions = extract_mentions(&stored.content, &participants, user.id);
                    if !mentions.is_empty() {
                        if let Err(error) =
                            state.record_message_mentions(stored.id, &mentions).await
                        {
                            tracing::warn!("record message mentions failed: {}", error);
                        }
                    }
                }
                Err(error) => {
                    tracing::error!("persist chat message failed: {}", error);
                    state
                        .broadcast(
                            room_id,
                            ChatMessage::System {
                                content: format!(
                                    "message from {} was not saved or broadcast",
                                    user.username
                                ),
                                members: None,
                                participants: None,
                            },
                        )
                        .await;
                }
            }
        }
        ChatMessage::Edit {
            message_id,
            content,
        } => {
            let Some(content) = normalize_message(content) else {
                return;
            };
            match state
                .edit_message(room_id, user.id, message_id, &content)
                .await
            {
                Ok(Some(edited_at)) => {
                    state
                        .broadcast(
                            room_id,
                            ChatMessage::MessageEdited {
                                message_id,
                                content,
                                edited_at,
                            },
                        )
                        .await;
                }
                Ok(None) => {}
                Err(error) => tracing::warn!("edit message failed: {}", error),
            }
        }
        ChatMessage::Typing { content, .. } => {
            state
                .broadcast(
                    room_id,
                    ChatMessage::Typing {
                        content: normalize_typing(content),
                        user_id: Some(user.id),
                        username: Some(user.username.clone()),
                    },
                )
                .await;
        }
        ChatMessage::Read { message_id } => {
            match state.store_read_cursor(room_id, user.id, message_id).await {
                Ok(true) => {
                    state
                        .broadcast(
                            room_id,
                            ChatMessage::ReadReceipt {
                                user_id: user.id,
                                username: user.username.clone(),
                                message_id,
                            },
                        )
                        .await;
                }
                Ok(false) => {}
                Err(error) => tracing::warn!("store read receipt failed: {}", error),
            }
        }
        ChatMessage::Poke { target_user_id } => {
            if target_user_id == user.id {
                return;
            }
            let members = state.connected_members(room_id).await;
            if !members.iter().any(|member| member.user_id == target_user_id) {
                return;
            }
            if !state
                .check_action_cooldown(room_id, user.id, target_user_id, state.poke_cooldown())
                .await
            {
                return;
            }
            state
                .broadcast(
                    room_id,
                    ChatMessage::System {
                        content: format!("poke:{}:{}", user.id, target_user_id),
                        members: None,
                        participants: None,
                    },
                )
                .await;
        }
        ChatMessage::Recall { message_id } => {
            match state.recall_message(room_id, user.id, message_id).await {
                Ok(Some(recalled_at)) => {
                    state
                        .broadcast(
                            room_id,
                            ChatMessage::MessageRecalled {
                                message_id,
                                recalled_at,
                            },
                        )
                        .await;
                }
                Ok(None) => {}
                Err(error) => tracing::warn!("recall message failed: {}", error),
            }
        }
        _ => {}
    }
}
