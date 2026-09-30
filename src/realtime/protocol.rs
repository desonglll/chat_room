//! Conversion, routing, and cursor helpers for outbound WebSocket messages.

use uuid::Uuid;

use crate::message_store::MessageCursor;
use crate::models::{ChatMember, ChatMessage, StoredMessage, UserStatus, UserStatusEntry};

/// Whether one broadcast frame may be delivered to `viewer`'s connection.
///
/// Almost every frame is chat-wide. The exception is `draft_updated` (TG-007/TG-008): a cloud
/// draft is broadcast on the chat channel but belongs to one account, so the transport keeps
/// it private to that account's own connections.
pub(crate) fn frame_visible_to(message: &ChatMessage, viewer: Uuid) -> bool {
    match message {
        ChatMessage::DraftUpdated { user_id, .. } => *user_id == viewer,
        _ => true,
    }
}

/// The `auth_ok.statuses` snapshot: `online` for currently connected accounts, `empty` for
/// every other active participant. TG-505 replaces `empty` with persisted, privacy-filtered
/// last-seen tiers; until then the server honestly reports "no information".
pub(crate) fn initial_statuses(
    connected: &[ChatMember],
    participants: &[ChatMember],
) -> Vec<UserStatusEntry> {
    participants
        .iter()
        .map(|participant| UserStatusEntry {
            user_id: participant.user_id,
            status: if connected
                .iter()
                .any(|member| member.user_id == participant.user_id)
            {
                UserStatus::Online
            } else {
                UserStatus::Empty
            },
        })
        .collect()
}

pub(crate) fn stored_message_to_chat(message: StoredMessage) -> ChatMessage {
    ChatMessage::Broadcast {
        message_id: message.id,
        client_message_id: message.client_message_id,
        sender_id: message.sender_id,
        sender: message.sender,
        sender_avatar: message.sender_avatar,
        content: message.content,
        attachment: message.attachment,
        reply_to: message.reply_to,
        recalled_at: message.recalled_at,
        edited_at: message.edited_at,
        timestamp: message.created_at,
        favorite_id: message.favorite_id,
        forwarded_from: message.forwarded_from,
        reactions: message.reactions,
        poll: message.poll,
    }
}

pub(crate) fn advance_message_cursor(cursor: &mut Option<MessageCursor>, message: &ChatMessage) {
    let ChatMessage::Broadcast {
        message_id,
        timestamp,
        ..
    } = message
    else {
        return;
    };
    let next = MessageCursor {
        created_at: *timestamp,
        id: *message_id,
    };
    if cursor
        .as_ref()
        .is_none_or(|current| (next.created_at, next.id) > (current.created_at, current.id))
    {
        *cursor = Some(next);
    }
}
