//! Mutations triggered by authenticated client WebSocket frames.

use uuid::Uuid;

use crate::models::{ChatMessage, User};
use crate::state::SharedState;
use crate::ws_auth::{normalize_message, normalize_typing};

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
            state
                .broadcast(
                    room_id,
                    ChatMessage::Typing {
                        content: String::new(),
                        user_id: Some(user.id),
                        username: Some(user.username.clone()),
                    },
                )
                .await;
            if let Err(error) = state
                .store_message(
                    room_id,
                    user.id,
                    &user.username,
                    &user.avatar_emoji,
                    &content,
                    reply_to,
                )
                .await
            {
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
