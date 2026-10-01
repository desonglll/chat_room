//! TG-907 keys and answers: the Contacts tab, pin/unpin and forward from the open chat.

use crossterm::event::{KeyCode, KeyEvent};
use uuid::Uuid;

use super::{
    input::TextField,
    model::{Action, App, Dialog, PromptKind, View},
    social::{pin_lines, ContactRow, SocialAction, SocialEvent},
};

fn social(action: SocialAction) -> Vec<Action> {
    vec![Action::Social(action)]
}

impl App {
    pub(super) fn handle_contacts_key(&mut self, key: KeyEvent) -> Vec<Action> {
        let rows = self.social.contacts.rows();
        if super::navigation::move_selection(&mut self.social.index, rows.len(), key) {
            return Vec::new();
        }
        let selected = rows.get(self.social.index).cloned();
        let plain = super::navigation::is_plain(key);
        match (key.code, selected) {
            (KeyCode::Enter, Some(ContactRow::Friend { user_id, name, .. })) => {
                self.busy = true;
                self.status = format!("Opening chat with {name}...");
                return social(SocialAction::OpenDirectChat(user_id));
            }
            (KeyCode::Char('a'), Some(ContactRow::Request { user_id, .. })) if plain => {
                return social(SocialAction::Respond {
                    user_id,
                    accept: true,
                });
            }
            (KeyCode::Char('x'), Some(ContactRow::Request { user_id, .. })) if plain => {
                return social(SocialAction::Respond {
                    user_id,
                    accept: false,
                });
            }
            (KeyCode::Char('n'), _) if plain => {
                self.dialog = Some(Dialog::Prompt {
                    title: "Add contact (@username)".into(),
                    kind: PromptKind::AddContact,
                    input: TextField::default(),
                });
            }
            _ => {}
        }
        Vec::new()
    }

    /// `P` in the message pane: pin the selected message, or unpin it when it is pinned.
    pub(super) fn toggle_pin_selected(&mut self) -> Vec<Action> {
        let (Some(room_id), Some(message)) = (self.active_chat, self.selected_message()) else {
            return Vec::new();
        };
        let message_id = message.id;
        let pinned = !self.social.pins.iter().any(|(id, _)| *id == message_id);
        social(SocialAction::SetPinned {
            room_id,
            message_id,
            pinned,
        })
    }

    /// `F` in the message pane: ask which chat to forward the selected message to.
    pub(super) fn prompt_forward_selected(&mut self) {
        if let Some(message) = self.selected_message() {
            self.dialog = Some(Dialog::Prompt {
                title: "Forward to chat (title)".into(),
                kind: PromptKind::Forward(message.id),
                input: TextField::default(),
            });
        }
    }

    pub(super) fn add_contact_action(&mut self, username: String) -> Vec<Action> {
        if username.trim().trim_start_matches('@').is_empty() {
            return Vec::new();
        }
        social(SocialAction::AddContact(username))
    }

    /// The conversation whose title matches `query` (exact, then prefix, case-insensitive).
    pub(super) fn forward_action(&mut self, message_id: Uuid, query: &str) -> Vec<Action> {
        let wanted = query.trim().to_lowercase();
        let found = self
            .conversations
            .iter()
            .find(|chat| chat.title.to_lowercase() == wanted)
            .or_else(|| {
                self.conversations
                    .iter()
                    .find(|chat| chat.title.to_lowercase().starts_with(&wanted))
            });
        match found {
            Some(chat) if !wanted.is_empty() => social(SocialAction::Forward {
                message_id,
                target: chat.room_id,
                title: chat.title.clone(),
            }),
            _ => {
                self.status = format!("No chat named \"{}\"", query.trim());
                Vec::new()
            }
        }
    }

    pub(super) fn apply_social_event(&mut self, event: SocialEvent) -> Vec<Action> {
        self.busy = false;
        match event {
            SocialEvent::Contacts(Ok(data)) => {
                let requests = data.requests.len();
                self.social.contacts = data;
                let total = self.social.contacts.rows().len();
                self.social.index = self.social.index.min(total.saturating_sub(1));
                self.status = format!(
                    "{} contacts, {requests} request(s)",
                    self.social.contacts.friends.len()
                );
            }
            SocialEvent::ContactsChanged(Ok(message)) => {
                self.status = message;
                return social(SocialAction::LoadContacts);
            }
            SocialEvent::DirectChatOpened(Ok(room_id)) => {
                self.view = View::Chats;
                let mut actions = vec![Action::LoadConversations];
                actions.extend(self.connect_chat(room_id, None));
                return actions;
            }
            SocialEvent::Pins {
                room_id,
                result: Ok(pins),
            } if self.active_chat == Some(room_id) => {
                self.social.pins = pin_lines(pins);
            }
            SocialEvent::Pins { .. } => {}
            SocialEvent::PinChanged {
                room_id,
                pinned,
                result: Ok(()),
            } => {
                self.status = if pinned {
                    "Message pinned"
                } else {
                    "Message unpinned"
                }
                .into();
                return social(SocialAction::LoadPins(room_id));
            }
            SocialEvent::Forwarded {
                title,
                result: Ok(result),
            } => {
                self.status = match (result.forwarded_message_id, result.skipped_reason) {
                    (Some(_), _) => format!("Forwarded to {title}"),
                    (None, Some(reason)) if reason == "voice_messages_restricted" => {
                        format!("{title} does not accept voice messages from you")
                    }
                    (None, reason) => format!(
                        "Not forwarded to {title}: {}",
                        reason.unwrap_or_else(|| "refused".into())
                    ),
                };
            }
            SocialEvent::Contacts(Err(error))
            | SocialEvent::ContactsChanged(Err(error))
            | SocialEvent::DirectChatOpened(Err(error))
            | SocialEvent::PinChanged {
                result: Err(error), ..
            }
            | SocialEvent::Forwarded {
                result: Err(error), ..
            } => {
                self.status = error.to_string();
            }
        }
        Vec::new()
    }
}
