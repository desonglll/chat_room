//! Conversion and cursor helpers for outbound WebSocket messages.

use crate::message_store::MessageCursor;
use crate::models::{ChatMessage, StoredMessage};

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
        forwarded_from: message.forwarded_from,
        reactions: message.reactions,
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
