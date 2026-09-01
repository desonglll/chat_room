//! View-specific keyboard behavior.

use crossterm::event::{KeyCode, KeyEvent};

use super::{
    input::TextField,
    model::{Action, App, ConfirmKind, Dialog, Focus},
};

impl App {
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
        if super::navigation::move_selection(&mut self.search_index, self.search_results.len(), key)
        {
            return Vec::new();
        }
        match key.code {
            KeyCode::Char('/') | KeyCode::Char('i') if super::navigation::is_plain(key) => {
                self.focus = Focus::Input;
            }
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
            key,
        ) {
            return Vec::new();
        }
        match key.code {
            KeyCode::Char('a') if super::navigation::is_plain(key) => {
                return vec![Action::ReadAllNotifications];
            }
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
        if super::navigation::move_selection(&mut self.favorite_index, self.favorites.len(), key) {
            return Vec::new();
        }
        match key.code {
            KeyCode::Char('n') if super::navigation::is_plain(key) => {
                self.dialog = Some(Dialog::FavoriteEditor {
                    id: None,
                    version: 1,
                    title: TextField::default(),
                    content: TextField::default(),
                    field: 0,
                })
            }
            KeyCode::Char('e') if super::navigation::is_plain(key) => {
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
            KeyCode::Char('d') if super::navigation::is_plain(key) => {
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
        if super::navigation::move_selection(&mut self.ai_thread_index, self.ai_threads.len(), key)
        {
            return Vec::new();
        }
        match key.code {
            KeyCode::Char('i') if super::navigation::is_plain(key) => self.focus = Focus::Input,
            KeyCode::Char('n') if super::navigation::is_plain(key) => {
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
