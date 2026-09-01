//! Session lifecycle and realtime chat event handling.

use crate::{
    client_api::MessagePreview,
    client_auth::{config_path, save_config_to, UserConfig},
    client_chat::{ChatCommand, ChatEvent, ChatMessage, DeliveryState},
};

use super::model::{Action, App, Screen};

impl App {
    pub(super) fn queue_outgoing_message(
        &mut self,
        content: String,
        reply_to: Option<uuid::Uuid>,
    ) -> ChatCommand {
        let client_message_id = uuid::Uuid::new_v4();
        let message = ChatMessage {
            id: client_message_id,
            client_message_id: Some(client_message_id),
            sender: self.username.clone(),
            content: content.clone(),
            attachment: None,
            timestamp: chrono::Utc::now().to_rfc3339(),
            recalled: false,
            edited: false,
            delivery: DeliveryState::Sending,
        };
        self.update_conversation_preview(&message);
        self.messages.push(message);
        self.message_index = self.messages.len().saturating_sub(1);
        self.status = "Sending message...".into();
        ChatCommand::Send {
            content,
            reply_to,
            client_message_id,
        }
    }

    pub(super) fn fail_outgoing_message(&mut self, client_message_id: uuid::Uuid, reason: &str) {
        if let Some(message) = self.messages.iter_mut().find(|message| {
            message.client_message_id == Some(client_message_id)
                && message.delivery == DeliveryState::Sending
        }) {
            message.delivery = DeliveryState::Failed;
        }
        self.status = reason.into();
    }

    pub(super) fn session_validated(
        &mut self,
        result: crate::client_api::ApiResult<()>,
    ) -> Vec<Action> {
        self.busy = false;
        match result {
            Ok(()) => {
                self.status = format!("Signed in as {}", self.username);
                let mut actions = vec![Action::LoadConversations];
                if let Some((room_id, password)) = self.initial_room.take() {
                    actions.push(Action::ConnectRoom {
                        room_id,
                        password,
                        target_message: None,
                    });
                }
                actions
            }
            Err(_) => {
                self.clear_session();
                self.status = "Saved session expired; sign in again".into();
                Vec::new()
            }
        }
    }

    pub(super) fn authenticated(
        &mut self,
        result: crate::client_api::ApiResult<crate::client_api::AuthSession>,
    ) -> Vec<Action> {
        self.busy = false;
        match result {
            Ok(session) => {
                self.username = session.user.username;
                self.token = Some(session.token);
                self.screen = Screen::Main;
                self.auth_password.clear();
                let config = UserConfig {
                    username: self.username.clone(),
                    token: self.token,
                };
                if let Err(error) = save_config_to(&config_path(), &config) {
                    self.status = format!("Signed in, but could not save session: {error}");
                } else {
                    self.status = format!("Signed in as {}", self.username);
                }
                let mut actions = vec![Action::LoadConversations];
                if let Some((room_id, password)) = self.initial_room.take() {
                    actions.push(Action::ConnectRoom {
                        room_id,
                        password,
                        target_message: None,
                    });
                }
                actions
            }
            Err(error) => {
                self.status = error.to_string();
                Vec::new()
            }
        }
    }

    pub(super) fn apply_chat_event(
        &mut self,
        room_id: uuid::Uuid,
        event: ChatEvent,
    ) -> Vec<Action> {
        if self.active_room != Some(room_id) {
            return Vec::new();
        }
        match event {
            ChatEvent::Message(message) => {
                self.update_conversation_preview(&message);
                let existing = self.messages.iter().position(|candidate| {
                    candidate.id == message.id
                        || message.client_message_id.is_some_and(|client_message_id| {
                            candidate.client_message_id == Some(client_message_id)
                        })
                });
                if let Some(index) = existing {
                    let confirms_outgoing =
                        message.client_message_id.is_some_and(|client_message_id| {
                            self.messages[index].client_message_id == Some(client_message_id)
                        });
                    self.messages[index] = message;
                    self.message_index = index;
                    if confirms_outgoing {
                        self.status = "Message sent".into();
                    }
                } else {
                    self.messages.push(message);
                    self.message_index = self.messages.len().saturating_sub(1);
                }
            }
            ChatEvent::HistoryComplete => {
                if let Some(target) = self.pending_message.take() {
                    if let Some(index) = self
                        .messages
                        .iter()
                        .position(|message| message.id == target)
                    {
                        self.message_index = index;
                    }
                }
                self.status = format!("Connected to {}", self.active_room_name);
                if let Some(message) = self.messages.last() {
                    return vec![Action::Chat(ChatCommand::Read(message.id))];
                }
            }
            ChatEvent::System(content) => self.status = content,
            ChatEvent::Edited {
                message_id,
                content,
            } => {
                if let Some(message) = self.messages.iter_mut().find(|item| item.id == message_id) {
                    message.content = content;
                    message.edited = true;
                }
                return vec![Action::LoadConversations];
            }
            ChatEvent::Recalled(message_id) => {
                if let Some(message) = self.messages.iter_mut().find(|item| item.id == message_id) {
                    message.recalled = true;
                    message.content.clear();
                }
                return vec![Action::LoadConversations];
            }
            ChatEvent::ReactionChanged {
                message_id,
                emoji,
                active,
            } => {
                let short_id = &message_id.to_string()[..8];
                self.status = if active {
                    format!("Added reaction {emoji} to #{short_id}")
                } else {
                    format!("Removed reaction {emoji} from #{short_id}")
                };
            }
            ChatEvent::Typing(username) => self.typing_user = username,
            ChatEvent::Closed => {
                self.fail_sending_messages();
                self.chat = None;
                self.status = "Connection closed".into();
            }
            ChatEvent::Error(error) => {
                self.fail_sending_messages();
                self.status = error;
            }
        }
        Vec::new()
    }

    fn update_conversation_preview(&mut self, message: &ChatMessage) {
        if let Some(conversation) = self
            .conversations
            .iter_mut()
            .find(|conversation| Some(conversation.room_id) == self.active_room)
        {
            conversation.last_message = Some(MessagePreview {
                sender: message.sender.clone(),
                content: message.content.clone(),
                recalled: message.recalled,
            });
            conversation.unread_count = 0;
        }
    }

    fn fail_sending_messages(&mut self) {
        for message in &mut self.messages {
            if message.delivery == DeliveryState::Sending {
                message.delivery = DeliveryState::Failed;
            }
        }
    }

    pub(super) fn api_error(&mut self, error: crate::client_api::ApiError) -> Vec<Action> {
        self.busy = false;
        if error.is_unauthorized() {
            self.clear_session();
        }
        self.status = error.to_string();
        Vec::new()
    }

    pub(super) fn clear_session(&mut self) {
        self.screen = Screen::SignIn;
        self.token = None;
        self.chat = None;
        self.busy = false;
        let _ = save_config_to(&config_path(), &UserConfig::default());
    }
}
