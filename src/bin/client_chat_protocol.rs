//! Typed WebSocket commands and events for the terminal client.

use anyhow::{Context, Result};
use serde::Deserialize;
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::client_media::Attachment;

#[derive(Clone, Debug)]
pub struct ChatMessage {
    pub id: Uuid,
    pub client_message_id: Option<Uuid>,
    pub sender: String,
    pub content: String,
    pub attachment: Option<Attachment>,
    pub timestamp: String,
    pub recalled: bool,
    pub edited: bool,
    pub delivery: DeliveryState,
    /// TG-1103: poll, voice, sticker, location … as text-renderable parts.
    pub media: Box<crate::client_chat_media::MessageMedia>,
    /// TG-1205: what it replies to, its reactions, whether it was sent silently.
    pub extras: Box<crate::client_chat_extras::MessageExtras>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeliveryState {
    Sending,
    Sent,
    Failed,
}

#[derive(Clone, Debug)]
pub enum ChatEvent {
    Message(ChatMessage),
    HistoryComplete,
    System(String),
    Edited {
        message_id: Uuid,
        content: String,
    },
    Recalled(Uuid),
    ReactionChanged {
        message_id: Uuid,
        emoji: String,
        /// TG-1205: who reacted; lets the client keep per-emoji counts.
        user_id: Option<Uuid>,
        active: bool,
    },
    /// The whole typing line (`alice is recording a voice message`), `None` once it stops.
    Typing(Option<String>),
    /// TG-907: a pin or unpin in this chat; the client re-reads the pins.
    PinsChanged,
    /// TG-1103: new poll counts for one message.
    PollUpdated {
        message_id: Uuid,
        poll: crate::client_chat_media::Poll,
    },
    Closed,
    Error(String),
}

#[derive(Clone, Debug)]
pub enum ChatCommand {
    Send {
        content: String,
        reply_to: Option<Uuid>,
        client_message_id: Uuid,
        /// TG-1205: deliver without notifications.
        silent: bool,
        /// TG-1205: the quoted part of `reply_to`.
        reply_quote: Option<String>,
    },
    Edit {
        message_id: Uuid,
        content: String,
    },
    Recall(Uuid),
    React {
        message_id: Uuid,
        emoji: String,
        active: bool,
    },
    Read(Uuid),
    Typing(String),
    Close,
}

impl ChatCommand {
    pub fn client_message_id(&self) -> Option<Uuid> {
        match self {
            Self::Send {
                client_message_id, ..
            } => Some(*client_message_id),
            _ => None,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
// This transport enum intentionally mirrors the JSON protocol without boxed wire fields.
#[allow(clippy::large_enum_variant)]
pub(super) enum ServerMessage {
    #[serde(rename = "auth_ok")]
    AuthOk { room_name: String },
    #[serde(rename = "auth_fail")]
    AuthFail { reason: String },
    #[serde(rename = "history_complete")]
    HistoryComplete,
    #[serde(rename = "broadcast")]
    Broadcast {
        message_id: Uuid,
        #[serde(default)]
        client_message_id: Option<Uuid>,
        sender: String,
        content: String,
        #[serde(default)]
        attachment: Option<Attachment>,
        timestamp: String,
        #[serde(default)]
        recalled_at: Option<String>,
        #[serde(default)]
        edited_at: Option<String>,
        #[serde(flatten)]
        media: crate::client_chat_media::MessageMedia,
        #[serde(flatten)]
        extras: crate::client_chat_extras::MessageExtras,
    },
    /// TG-1103: a poll's new counts (and, for the voter's own connections, `chosen`).
    #[serde(rename = "poll_updated")]
    PollUpdated {
        message_id: Uuid,
        poll: crate::client_chat_media::Poll,
    },
    #[serde(rename = "system")]
    System { content: String },
    #[serde(rename = "message_edited")]
    MessageEdited { message_id: Uuid, content: String },
    #[serde(rename = "message_recalled")]
    MessageRecalled { message_id: Uuid },
    #[serde(rename = "reaction_changed")]
    ReactionChanged {
        message_id: Uuid,
        emoji: String,
        #[serde(default)]
        user_id: Option<Uuid>,
        active: bool,
    },
    #[serde(rename = "typing")]
    Typing {
        content: String,
        #[serde(default)]
        username: Option<String>,
        #[serde(default)]
        action: Option<String>,
    },
    #[serde(rename = "presence")]
    Presence {},
    #[serde(rename = "pins_changed")]
    PinsChanged {},
    #[serde(rename = "read_receipt")]
    ReadReceipt {},
    /// Catch-all for frame kinds this frozen client does not know (the TG-007 extensions and
    /// anything later milestones add). Ignored instead of surfacing a decode error in the UI.
    #[serde(other)]
    Unknown,
}

pub(super) fn command_frame(command: ChatCommand) -> serde_json::Value {
    match command {
        ChatCommand::Send {
            content,
            reply_to,
            client_message_id,
            silent,
            reply_quote,
        } => {
            let mut frame = serde_json::json!({
                "type": "message",
                "content": content,
                "reply_to": reply_to,
                "client_message_id": client_message_id
            });
            if silent {
                frame["silent"] = true.into();
            }
            if let Some(text) = reply_quote.filter(|_| reply_to.is_some()) {
                frame["reply_quote"] = serde_json::json!({ "text": text });
            }
            frame
        }
        ChatCommand::Edit {
            message_id,
            content,
        } => serde_json::json!({ "type": "edit", "message_id": message_id, "content": content }),
        ChatCommand::Recall(message_id) => {
            serde_json::json!({ "type": "recall", "message_id": message_id })
        }
        ChatCommand::React {
            message_id,
            emoji,
            active,
        } => serde_json::json!({
            "type": "reaction",
            "message_id": message_id,
            "emoji": emoji,
            "active": active
        }),
        ChatCommand::Read(message_id) => {
            serde_json::json!({ "type": "read", "message_id": message_id })
        }
        ChatCommand::Typing(content) => {
            serde_json::json!({ "type": "typing", "content": content })
        }
        ChatCommand::Close => unreachable!("close is handled before serialization"),
    }
}

pub(super) fn decode_server_message(text: &str) -> Result<ServerMessage> {
    serde_json::from_str(text).context("invalid server message")
}

pub(super) fn emit_server_event(sender: &mpsc::UnboundedSender<ChatEvent>, message: ServerMessage) {
    let event = match message {
        ServerMessage::Broadcast {
            message_id,
            client_message_id,
            sender: author,
            content,
            attachment,
            timestamp,
            recalled_at,
            edited_at,
            media,
            extras,
        } => ChatEvent::Message(ChatMessage {
            id: message_id,
            client_message_id,
            sender: clean(&author),
            content: clean_multiline(&content),
            attachment,
            timestamp,
            recalled: recalled_at.is_some(),
            edited: edited_at.is_some(),
            delivery: DeliveryState::Sent,
            media: Box::new(media),
            extras: Box::new(extras),
        }),
        ServerMessage::PollUpdated { message_id, poll } => {
            ChatEvent::PollUpdated { message_id, poll }
        }
        ServerMessage::HistoryComplete => ChatEvent::HistoryComplete,
        ServerMessage::System { content } => ChatEvent::System(clean_multiline(&content)),
        ServerMessage::MessageEdited {
            message_id,
            content,
        } => ChatEvent::Edited {
            message_id,
            content: clean_multiline(&content),
        },
        ServerMessage::MessageRecalled { message_id } => ChatEvent::Recalled(message_id),
        ServerMessage::ReactionChanged {
            message_id,
            emoji,
            user_id,
            active,
        } => ChatEvent::ReactionChanged {
            message_id,
            emoji: clean(&emoji),
            user_id,
            active,
        },
        ServerMessage::Typing {
            content,
            username,
            action,
        } => {
            let action = action.unwrap_or_default();
            // Empty content stops plain typing; the media actions carry no draft text.
            let active = action != "cancel"
                && (!content.is_empty() || !action.is_empty() && action != "typing");
            ChatEvent::Typing(active.then(|| {
                let phrase = crate::client_chat_extras::typing_phrase(&action);
                format!("{} {phrase}", clean(&username.unwrap_or_default()))
            }))
        }
        ServerMessage::AuthFail { reason } => ChatEvent::Error(clean(&reason)),
        ServerMessage::PinsChanged {} => ChatEvent::PinsChanged,
        ServerMessage::AuthOk { .. }
        | ServerMessage::Presence {}
        | ServerMessage::ReadReceipt {}
        | ServerMessage::Unknown => return,
    };
    let _ = sender.send(event);
}

fn clean(value: &str) -> String {
    value
        .chars()
        .filter(|character| !character.is_control())
        .collect()
}

fn clean_multiline(value: &str) -> String {
    value
        .chars()
        .filter(|character| !character.is_control() || matches!(character, '\n' | '\t'))
        .collect()
}

#[cfg(test)]
#[path = "client_chat_protocol_tests.rs"]
mod tests;
