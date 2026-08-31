//! Global keyboard transitions and navigation for the TUI state.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::model::{Action, App, AuthMode, Dialog, Focus, PromptKind, Screen, View};

impl App {
    pub fn handle_key(&mut self, key: KeyEvent) -> Vec<Action> {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            return vec![Action::Quit];
        }
        if self.dialog.is_some() {
            return self.handle_dialog_key(key);
        }
        if self.screen == Screen::SignIn {
            return self.handle_sign_in_key(key);
        }
        if key.code == KeyCode::F(1)
            || (self.focus != Focus::Input && key.code == KeyCode::Char('?'))
        {
            self.dialog = Some(Dialog::Help);
            return Vec::new();
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('l') {
            self.busy = true;
            self.status = "Logging out...".into();
            return vec![Action::Logout];
        }
        if self.focus != Focus::Input {
            if key.code == KeyCode::Char('q') {
                return vec![Action::Quit];
            }
            if let KeyCode::Char(number @ '1'..='5') = key.code {
                let index = number as usize - '1' as usize;
                return self.select_view(View::ALL[index]);
            }
        }
        match key.code {
            KeyCode::Tab => self.focus = self.next_focus(false),
            KeyCode::BackTab => self.focus = self.next_focus(true),
            KeyCode::Char('r') if self.focus != Focus::Input => return self.refresh_actions(),
            _ => {}
        }
        match self.view {
            View::Chats => self.handle_chats_key(key),
            View::Search => self.handle_search_key(key),
            View::Notifications => self.handle_notifications_key(key),
            View::Favorites => self.handle_favorites_key(key),
            View::Ai => self.handle_ai_key(key),
        }
    }

    fn handle_sign_in_key(&mut self, key: KeyEvent) -> Vec<Action> {
        match key.code {
            KeyCode::F(2) => {
                self.auth_mode = match self.auth_mode {
                    AuthMode::Login => AuthMode::Register,
                    AuthMode::Register => AuthMode::Login,
                };
            }
            KeyCode::Tab | KeyCode::BackTab => self.auth_field = 1 - self.auth_field,
            KeyCode::Enter if self.auth_field == 0 => self.auth_field = 1,
            KeyCode::Enter => {
                if self.auth_username.value().trim().is_empty()
                    || self.auth_password.value().is_empty()
                {
                    self.status = "Username and password are required".into();
                    return Vec::new();
                }
                self.busy = true;
                self.status = "Authenticating...".into();
                return vec![Action::Authenticate {
                    register: self.auth_mode == AuthMode::Register,
                    username: self.auth_username.value().trim().to_string(),
                    password: self.auth_password.value().to_string(),
                }];
            }
            _ => {
                if self.auth_field == 0 {
                    self.auth_username.handle_key(key);
                } else {
                    self.auth_password.handle_key(key);
                }
            }
        }
        Vec::new()
    }

    pub(super) fn connect_selected(&mut self, target_message: Option<uuid::Uuid>) -> Vec<Action> {
        let Some(room_id) = self.selected_conversation().map(|room| room.room_id) else {
            return Vec::new();
        };
        self.connect_room(room_id, target_message)
    }

    pub(super) fn connect_room(
        &mut self,
        room_id: uuid::Uuid,
        target_message: Option<uuid::Uuid>,
    ) -> Vec<Action> {
        let needs_password = self
            .conversations
            .iter()
            .find(|room| room.room_id == room_id)
            .and_then(|room| room.group.as_ref())
            .is_some_and(|group| group.has_password);
        let password = self.room_passwords.get(&room_id).cloned();
        if needs_password && password.is_none() {
            self.dialog = Some(Dialog::Prompt {
                title: "Room password".into(),
                kind: PromptKind::RoomPassword {
                    room_id,
                    target_message,
                },
                input: super::input::TextField::password(),
            });
            return Vec::new();
        }
        self.busy = true;
        self.status = "Connecting...".into();
        vec![Action::ConnectRoom {
            room_id,
            password,
            target_message,
        }]
    }

    fn select_view(&mut self, view: View) -> Vec<Action> {
        self.view = view;
        self.focus = Focus::List;
        self.refresh_actions()
    }

    fn refresh_actions(&self) -> Vec<Action> {
        match self.view {
            View::Chats => vec![Action::LoadConversations],
            View::Search if self.search_input.value().trim().is_empty() => Vec::new(),
            View::Search => vec![Action::Search(self.search_input.value().trim().into())],
            View::Notifications => vec![Action::LoadNotifications],
            View::Favorites => vec![Action::LoadFavorites],
            View::Ai => vec![Action::LoadAiThreads],
        }
    }

    fn next_focus(&self, reverse: bool) -> Focus {
        let order: &[Focus] = match self.view {
            View::Chats => &[Focus::List, Focus::Content, Focus::Input],
            View::Search | View::Ai => &[Focus::List, Focus::Input],
            View::Notifications | View::Favorites => return Focus::List,
        };
        let current = order
            .iter()
            .position(|focus| *focus == self.focus)
            .unwrap_or(0);
        if reverse {
            order[(current + order.len() - 1) % order.len()]
        } else {
            order[(current + 1) % order.len()]
        }
    }
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    use super::*;
    use crate::{
        client_api::{Conversation, ConversationPreferences},
        client_auth::UserConfig,
    };

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn left_then_down_and_enter_switches_conversation() {
        let first_room = uuid::Uuid::new_v4();
        let next_room = uuid::Uuid::new_v4();
        let mut app = App::new("http://localhost".into(), UserConfig::default(), None);
        app.screen = Screen::Main;
        app.focus = Focus::Content;
        app.active_room = Some(first_room);
        app.conversations = [first_room, next_room]
            .into_iter()
            .map(|room_id| Conversation {
                room_id,
                kind: "group".into(),
                title: room_id.to_string(),
                unread_count: 0,
                group: None,
                preferences: ConversationPreferences::default(),
                last_message: None,
            })
            .collect();

        assert!(app.handle_key(key(KeyCode::Left)).is_empty());
        assert_eq!(app.focus, Focus::List);
        assert!(app.handle_key(key(KeyCode::Char('j'))).is_empty());
        let actions = app.handle_key(key(KeyCode::Enter));

        assert!(matches!(
            actions.as_slice(),
            [Action::ConnectRoom { room_id, .. }] if *room_id == next_room
        ));
    }

    #[test]
    fn question_mark_opens_and_escape_closes_help() {
        let mut app = App::new("http://localhost".into(), UserConfig::default(), None);
        app.screen = Screen::Main;

        assert!(app.handle_key(key(KeyCode::Char('?'))).is_empty());
        assert!(matches!(app.dialog, Some(Dialog::Help)));
        assert!(app.handle_key(key(KeyCode::Esc)).is_empty());
        assert!(app.dialog.is_none());
    }
}
