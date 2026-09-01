//! WebSocket room and account authentication rules.

use sha2::{Digest, Sha256};

use crate::models::{ChatMessage, Room, User};
use crate::state::SharedState;

const MAX_MESSAGE_CHARS: usize = 4096;
const MAX_PASSWORD_CHARS: usize = 256;

pub(crate) struct AuthenticatedUser {
    pub user: User,
    pub session_id: uuid::Uuid,
}

pub(crate) async fn authenticate(
    state: &SharedState,
    room: &Room,
    message: ChatMessage,
) -> Result<AuthenticatedUser, String> {
    let (token, supplied_password) = match message {
        ChatMessage::Join { token } => (token, None),
        ChatMessage::Auth { token, password } => (token, Some(password)),
        _ => return Err("first message must be join or auth".into()),
    };

    let user = state
        .session_user(token)
        .await
        .map_err(|error| {
            tracing::error!("validate WebSocket session failed: {}", error);
            "authentication unavailable".to_string()
        })?
        .ok_or_else(|| "login required".to_string())?;
    if room.has_password {
        let is_active_member = state
            .membership_identity(room.id, user.id)
            .await
            .map_err(|error| {
                tracing::error!("load WebSocket membership failed: {}", error);
                "authentication unavailable".to_string()
            })?
            .is_some_and(|(status, _)| status == "active");
        if !is_active_member {
            let password = supplied_password
                .ok_or_else(|| "this room requires a password - send auth, not join".to_string())?;
            if password.chars().count() > MAX_PASSWORD_CHARS {
                return Err("password too long".into());
            }
            let mut hasher = Sha256::new();
            hasher.update(password.as_bytes());
            if hex::encode(hasher.finalize()) != room.password_hash {
                return Err("wrong password".into());
            }
        }
    }
    Ok(AuthenticatedUser {
        user,
        session_id: token,
    })
}

pub(crate) fn normalize_message(content: String) -> Option<String> {
    let content = content.trim().to_string();
    if content.is_empty() || content.chars().count() > MAX_MESSAGE_CHARS {
        return None;
    }
    Some(content)
}

pub(crate) fn normalize_typing(content: String) -> String {
    content
        .chars()
        .filter(|character| !character.is_control() || matches!(character, '\n' | '\t'))
        .take(512)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_and_limits_messages() {
        assert_eq!(normalize_message(" hello \n".into()).unwrap(), "hello");
        assert!(normalize_message(" \n".into()).is_none());
        assert!(normalize_message("x".repeat(MAX_MESSAGE_CHARS + 1)).is_none());
    }
}
