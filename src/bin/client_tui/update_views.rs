//! View-specific keyboard behavior.

use crossterm::event::{KeyCode, KeyEvent};

use crate::{client_api::PreferencePatch, client_chat::ChatCommand};

use super::{
    input::TextField,
    model::{Action, App, ConfirmKind, Dialog, Focus, PromptKind},
};

impl App {
    pub(super) fn handle_chats_key(&mut self, key: KeyEvent) -> Vec<Action> {
        if self.focus == Focus::Input {
            return self.handle_chat_input(key);
        }
        let moved = if self.focus == Focus::List {
            super::navigation::move_selection(
                &mut self.conversation_index,
                self.conversations.len(),
                key.code,
            )
        } else {
            super::navigation::move_selection(
                &mut self.message_index,
                self.messages.len(),
                key.code,
            )
        };
        if moved {
            return Vec::new();
        }
        match key.code {
            KeyCode::Left | KeyCode::Char('h') => self.focus = Focus::List,
            KeyCode::Right | KeyCode::Char('l') => self.focus = Focus::Content,
            KeyCode::Enter if self.focus == Focus::List => return self.connect_selected(None),
            KeyCode::Char('i') => self.focus = Focus::Input,
            KeyCode::Char('n') => {
                self.dialog = Some(Dialog::CreateRoom {
                    name: TextField::default(),
                    password: TextField::password(),
                    field: 0,
                });
            }
            KeyCode::Char('g') => return vec![Action::LoadRooms],
            KeyCode::Char('p') if self.focus == Focus::List => {
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
            KeyCode::Char('a') if self.focus == Focus::List => {
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
            KeyCode::Char('m') if self.focus == Focus::List => {
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
            KeyCode::Char('t') if self.focus == Focus::List => {
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
            KeyCode::Char('u') if self.active_room.is_some() => {
                self.dialog = Some(Dialog::Prompt {
                    title: "Upload file".into(),
                    kind: PromptKind::Upload,
                    input: TextField::default(),
                });
            }
            KeyCode::Char('d') if self.focus == Focus::Content => {
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
            KeyCode::Char('f') if self.focus == Focus::Content => {
                if let Some(message) = self.selected_message() {
                    return vec![Action::FavoriteMessage(message.id)];
                }
            }
            KeyCode::Char('e') if self.focus == Focus::Content => {
                if let Some(message) = self.selected_message() {
                    self.dialog = Some(Dialog::Prompt {
                        title: "Edit message".into(),
                        kind: PromptKind::EditMessage(message.id),
                        input: TextField::new(message.content.clone()),
                    });
                }
            }
            KeyCode::Char('x') if self.focus == Focus::Content => {
                if let Some(message) = self.selected_message() {
                    self.dialog = Some(Dialog::Confirm {
                        title: "Recall selected message?".into(),
                        kind: ConfirmKind::RecallMessage(message.id),
                    });
                }
            }
            KeyCode::Char('+') if self.focus == Focus::Content => {
                if let Some(message) = self.selected_message() {
                    self.dialog = Some(Dialog::Prompt {
                        title: "Add reaction".into(),
                        kind: PromptKind::Reaction(message.id),
                        input: TextField::default(),
                    });
                }
            }
            KeyCode::Char('R') if self.focus == Focus::Content => {
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
                let content = self.compose.take().trim().to_string();
                if !content.is_empty() {
                    let reply_to = self.reply_to.take();
                    return vec![Action::Chat(ChatCommand::Send { content, reply_to })];
                }
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

    pub(super) fn handle_search_key(&mut self, key: KeyEvent) -> Vec<Action> {
        if self.focus == Focus::Input {
            match key.code {
                KeyCode::Esc => self.focus = Focus::List,
                KeyCode::Enter => {
                    let query = self.search_input.value().trim().to_string();
                    if !query.is_empty() {
                        self.busy = true;
                        return vec![Action::Search(query)];
                    }
                }
                _ => {
                    self.search_input.handle_key(key);
                }
            }
            return Vec::new();
        }
        if super::navigation::move_selection(
            &mut self.search_index,
            self.search_results.len(),
            key.code,
        ) {
            return Vec::new();
        }
        match key.code {
            KeyCode::Char('/') | KeyCode::Char('i') => self.focus = Focus::Input,
            KeyCode::Enter => {
                if let Some(result) = self.search_results.get(self.search_index) {
                    return self.connect_room(result.room_id, Some(result.message_id));
                }
            }
            _ => {}
        }
        Vec::new()
    }

    pub(super) fn handle_notifications_key(&mut self, key: KeyEvent) -> Vec<Action> {
        if super::navigation::move_selection(
            &mut self.notification_index,
            self.notifications.len(),
            key.code,
        ) {
            return Vec::new();
        }
        match key.code {
            KeyCode::Char('a') => return vec![Action::ReadAllNotifications],
            KeyCode::Enter => {
                let Some(item) = self.notifications.get(self.notification_index).cloned() else {
                    return Vec::new();
                };
                let mut actions = vec![Action::ReadNotification(item.id)];
                if item.source_available {
                    if let Some(room_id) = item.room_id {
                        actions.extend(self.connect_room(room_id, item.message_id));
                    }
                }
                return actions;
            }
            _ => {}
        }
        Vec::new()
    }

    pub(super) fn handle_favorites_key(&mut self, key: KeyEvent) -> Vec<Action> {
        if super::navigation::move_selection(
            &mut self.favorite_index,
            self.favorites.len(),
            key.code,
        ) {
            return Vec::new();
        }
        match key.code {
            KeyCode::Char('n') => {
                self.dialog = Some(Dialog::FavoriteEditor {
                    id: None,
                    version: 1,
                    title: TextField::default(),
                    content: TextField::default(),
                    field: 0,
                })
            }
            KeyCode::Char('e') => {
                if let Some(item) = self.selected_favorite() {
                    self.dialog = Some(Dialog::FavoriteEditor {
                        id: Some(item.id),
                        version: item.version,
                        title: TextField::new(item.title.clone()),
                        content: TextField::new(item.content.clone()),
                        field: 0,
                    });
                }
            }
            KeyCode::Char('d') => {
                if let Some(item) = self.selected_favorite() {
                    self.dialog = Some(Dialog::Confirm {
                        title: "Delete selected favorite?".into(),
                        kind: ConfirmKind::DeleteFavorite(item.id),
                    });
                }
            }
            KeyCode::Enter => {
                if let Some(item) = self.selected_favorite() {
                    if let Some(room_id) = item.source_room_id {
                        return self.connect_room(room_id, item.source_message_id);
                    }
                }
            }
            _ => {}
        }
        Vec::new()
    }

    pub(super) fn handle_ai_key(&mut self, key: KeyEvent) -> Vec<Action> {
        if self.focus == Focus::Input {
            match key.code {
                KeyCode::Esc => self.focus = Focus::List,
                KeyCode::Enter if !self.ai_running => {
                    let question = self.ai_input.take().trim().to_string();
                    if !question.is_empty() {
                        self.ai_running = true;
                        return vec![Action::AskAi {
                            thread_id: self.selected_ai_thread().map(|thread| thread.id),
                            question,
                            room_id: self.active_room,
                            room_password: self
                                .active_room
                                .and_then(|room_id| self.room_passwords.get(&room_id).cloned()),
                        }];
                    }
                }
                _ => {
                    self.ai_input.handle_key(key);
                }
            }
            return Vec::new();
        }
        if super::navigation::move_selection(
            &mut self.ai_thread_index,
            self.ai_threads.len(),
            key.code,
        ) {
            return Vec::new();
        }
        match key.code {
            KeyCode::Char('i') => self.focus = Focus::Input,
            KeyCode::Char('n') => {
                self.ai_thread_index = self.ai_threads.len();
                self.ai_messages.clear();
                self.focus = Focus::Input;
            }
            KeyCode::Enter => {
                if let Some(thread) = self.selected_ai_thread() {
                    return vec![Action::LoadAiMessages(thread.id)];
                }
            }
            _ => {}
        }
        Vec::new()
    }
}
