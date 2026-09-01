//! Conversation, message, and composer keyboard behavior.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::{client_api::PreferencePatch, client_chat::ChatCommand};

use super::{
    input::TextField,
    model::{Action, App, ConfirmKind, Dialog, Focus, PromptKind},
};

const MAX_MESSAGE_CHARS: usize = 4096;

impl App {
    pub(super) fn handle_chats_key(&mut self, key: KeyEvent) -> Vec<Action> {
        if self.focus == Focus::Input {
            return self.handle_chat_input(key);
        }
        let moved = if self.focus == Focus::List {
            super::navigation::move_selection(
                &mut self.conversation_index,
                self.conversations.len(),
                key,
            )
        } else {
            super::navigation::move_selection(&mut self.message_index, self.messages.len(), key)
        };
        if moved {
            return Vec::new();
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            match key.code {
                KeyCode::Char('b') => self.focus = Focus::List,
                KeyCode::Char('f') => self.focus = Focus::Content,
                _ => {}
            }
            if matches!(key.code, KeyCode::Char('b') | KeyCode::Char('f')) {
                return Vec::new();
            }
        }
        match key.code {
            KeyCode::Left => self.focus = Focus::List,
            KeyCode::Right => self.focus = Focus::Content,
            KeyCode::Char('h') if super::navigation::is_plain(key) => self.focus = Focus::List,
            KeyCode::Char('l') if super::navigation::is_plain(key) => self.focus = Focus::Content,
            KeyCode::Enter if self.focus == Focus::List => return self.connect_selected(None),
            KeyCode::Char('i') if super::navigation::is_plain(key) => self.focus = Focus::Input,
            KeyCode::Char('n') if super::navigation::is_plain(key) => {
                self.dialog = Some(Dialog::CreateRoom {
                    name: TextField::default(),
                    password: TextField::password(),
                    field: 0,
                });
            }
            KeyCode::Char('g') if super::navigation::is_plain(key) => {
                return vec![Action::LoadRooms];
            }
            KeyCode::Char('p') if self.focus == Focus::List && super::navigation::is_plain(key) => {
                if let Some(room) = self.selected_conversation() {
                    return vec![Action::UpdatePreferences {
                        room_id: room.room_id,
                        patch: PreferencePatch {
                            is_pinned: Some(!room.preferences.is_pinned),
                            ..PreferencePatch::default()
                        },
                    }];
                }
            }
            KeyCode::Char('a') if self.focus == Focus::List && super::navigation::is_plain(key) => {
                if let Some(room) = self.selected_conversation() {
                    return vec![Action::UpdatePreferences {
                        room_id: room.room_id,
                        patch: PreferencePatch {
                            is_archived: Some(!room.preferences.is_archived),
                            ..PreferencePatch::default()
                        },
                    }];
                }
            }
            KeyCode::Char('m') if self.focus == Focus::List && super::navigation::is_plain(key) => {
                if let Some(room) = self.selected_conversation() {
                    let level = match room.preferences.notification_level.as_str() {
                        "all" => "mentions",
                        "mentions" => "none",
                        _ => "all",
                    };
                    return vec![Action::UpdatePreferences {
                        room_id: room.room_id,
                        patch: PreferencePatch {
                            notification_level: Some(level.into()),
                            ..PreferencePatch::default()
                        },
                    }];
                }
            }
            KeyCode::Char('t') if self.focus == Focus::List && super::navigation::is_plain(key) => {
                if let Some(room) = self.selected_conversation() {
                    let muted_until = if room.preferences.muted_until.is_some() {
                        None
                    } else {
                        Some((chrono::Utc::now() + chrono::Duration::hours(1)).to_rfc3339())
                    };
                    return vec![Action::UpdatePreferences {
                        room_id: room.room_id,
                        patch: PreferencePatch {
                            muted_until: Some(muted_until),
                            ..PreferencePatch::default()
                        },
                    }];
                }
            }
            KeyCode::Char('u')
                if self.active_room.is_some() && super::navigation::is_plain(key) =>
            {
                self.dialog = Some(Dialog::Prompt {
                    title: "Upload file".into(),
                    kind: PromptKind::Upload,
                    input: TextField::default(),
                });
            }
            KeyCode::Char('d')
                if self.focus == Focus::Content && super::navigation::is_plain(key) =>
            {
                if let Some(attachment) = self
                    .selected_message()
                    .and_then(|message| message.attachment.clone())
                {
                    self.dialog = Some(Dialog::Prompt {
                        title: "Download to".into(),
                        kind: PromptKind::Download(attachment.clone()),
                        input: TextField::new(attachment.file_name),
                    });
                }
            }
            KeyCode::Char('f')
                if self.focus == Focus::Content && super::navigation::is_plain(key) =>
            {
                if let Some(message) = self.selected_message() {
                    return vec![Action::FavoriteMessage(message.id)];
                }
            }
            KeyCode::Char('e')
                if self.focus == Focus::Content && super::navigation::is_plain(key) =>
            {
                if let Some(message) = self.selected_message() {
                    self.dialog = Some(Dialog::Prompt {
                        title: "Edit message".into(),
                        kind: PromptKind::EditMessage(message.id),
                        input: TextField::new(message.content.clone()),
                    });
                }
            }
            KeyCode::Char('x')
                if self.focus == Focus::Content && super::navigation::is_plain(key) =>
            {
                if let Some(message) = self.selected_message() {
                    self.dialog = Some(Dialog::Confirm {
                        title: "Recall selected message?".into(),
                        kind: ConfirmKind::RecallMessage(message.id),
                    });
                }
            }
            KeyCode::Char('+')
                if self.focus == Focus::Content && super::navigation::is_plain(key) =>
            {
                if let Some(message) = self.selected_message() {
                    self.dialog = Some(Dialog::Prompt {
                        title: "Add reaction".into(),
                        kind: PromptKind::Reaction(message.id),
                        input: TextField::default(),
                    });
                }
            }
            KeyCode::Char('R')
                if self.focus == Focus::Content && super::navigation::is_plain(key) =>
            {
                self.reply_to = self.selected_message().map(|message| message.id);
                self.focus = Focus::Input;
            }
            _ => {}
        }
        Vec::new()
    }

    fn handle_chat_input(&mut self, key: KeyEvent) -> Vec<Action> {
        match key.code {
            KeyCode::Esc => {
                self.focus = Focus::Content;
                self.reply_to = None;
            }
            KeyCode::Enter => {
                let content = self.compose.value().trim().to_string();
                if content.is_empty() {
                    return Vec::new();
                }
                if self.chat.is_none() {
                    self.status = "Chat connection is not ready; draft preserved".into();
                    return Vec::new();
                }
                if content.chars().count() > MAX_MESSAGE_CHARS {
                    self.status = format!("Message is longer than {MAX_MESSAGE_CHARS} characters");
                    return Vec::new();
                }
                self.compose.clear();
                let reply_to = self.reply_to.take();
                let command = self.queue_outgoing_message(content, reply_to);
                return vec![Action::Chat(command)];
            }
            _ => {
                if self.compose.handle_key(key) {
                    return vec![Action::Chat(ChatCommand::Typing(
                        self.compose.value().to_string(),
                    ))];
                }
            }
        }
        Vec::new()
    }
}
