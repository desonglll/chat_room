//! Typed WebSocket commands and events for the terminal client.

use anyhow::{Context, Result};
use serde::Deserialize;
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::client_media::Attachment;

#[derive(Clone, Debug)]
pub struct ChatMessage {
    pub id: Uuid,
    pub sender: String,
    pub content: String,
    pub attachment: Option<Attachment>,
    pub timestamp: String,
    pub recalled: bool,
    pub edited: bool,
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
        active: bool,
    },
    Typing(Option<String>),
    Closed,
    Error(String),
}

#[derive(Clone, Debug)]
pub enum ChatCommand {
    Send {
        content: String,
        reply_to: Option<Uuid>,
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

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
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
        sender: String,
        content: String,
        #[serde(default)]
        attachment: Option<Attachment>,
        timestamp: String,
        #[serde(default)]
        recalled_at: Option<String>,
        #[serde(default)]
        edited_at: Option<String>,
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
        active: bool,
    },
    #[serde(rename = "typing")]
    Typing {
        content: String,
        #[serde(default)]
        username: Option<String>,
    },
    #[serde(rename = "presence")]
    Presence {},
    #[serde(rename = "read_receipt")]
    ReadReceipt {},
}

pub(super) fn command_frame(command: ChatCommand) -> serde_json::Value {
    match command {
        ChatCommand::Send { content, reply_to } => serde_json::json!({
            "type": "message",
            "content": content,
            "reply_to": reply_to,
            "client_message_id": Uuid::new_v4()
        }),
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
            sender: author,
            content,
            attachment,
            timestamp,
            recalled_at,
            edited_at,
        } => ChatEvent::Message(ChatMessage {
            id: message_id,
            sender: clean(&author),
            content: clean_multiline(&content),
            attachment,
            timestamp,
            recalled: recalled_at.is_some(),
            edited: edited_at.is_some(),
        }),
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
            active,
        } => ChatEvent::ReactionChanged {
            message_id,
            emoji: clean(&emoji),
            active,
        },
        ServerMessage::Typing { content, username } => {
            ChatEvent::Typing((!content.is_empty()).then(|| username.unwrap_or_default()))
        }
        ServerMessage::AuthFail { reason } => ChatEvent::Error(clean(&reason)),
        ServerMessage::AuthOk { .. }
        | ServerMessage::Presence {}
        | ServerMessage::ReadReceipt {} => return,
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
mod tests {
    use super::*;

    #[test]
    fn decodes_current_broadcast_contract_without_terminal_controls() {
        let message_id = Uuid::new_v4();
        let message = decode_server_message(
            &serde_json::json!({
                "type": "broadcast",
                "message_id": message_id,
                "client_message_id": null,
                "sender_id": null,
                "sender": "alice\u{1b}",
                "sender_avatar": "",
                "content": "hello\nworld",
                "attachment": null,
                "reply_to": null,
                "recalled_at": null,
                "edited_at": null,
                "timestamp": "2026-08-31T00:00:00Z",
                "favorite_id": null,
                "forwarded_from": null,
                "reactions": []
            })
            .to_string(),
        )
        .unwrap();
        let (sender, mut receiver) = mpsc::unbounded_channel();
        emit_server_event(&sender, message);
        let ChatEvent::Message(message) = receiver.try_recv().unwrap() else {
            panic!("expected chat message");
        };
        assert_eq!(message.sender, "alice");
        assert_eq!(message.content, "hello\nworld");
    }

    #[test]
    fn send_commands_have_idempotency_identity() {
        let frame = command_frame(ChatCommand::Send {
            content: "hello".into(),
            reply_to: None,
        });
        assert_eq!(frame["type"], "message");
        assert!(frame["client_message_id"].as_str().is_some());
    }
}
