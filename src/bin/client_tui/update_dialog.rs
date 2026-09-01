//! Modal form and confirmation behavior.

use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent};

use super::model::{Action, App, ConfirmKind, Dialog, PromptKind};

impl App {
    pub(super) fn handle_dialog_key(&mut self, key: KeyEvent) -> Vec<Action> {
        let cancel = key.code == KeyCode::Esc
            || (key
                .modifiers
                .contains(crossterm::event::KeyModifiers::CONTROL)
                && key.code == KeyCode::Char('g'));
        if matches!(self.dialog, Some(Dialog::Help)) {
            if cancel
                || matches!(key.code, KeyCode::Enter | KeyCode::F(1))
                || (matches!(key.code, KeyCode::Char('?') | KeyCode::Char('q'))
                    && super::navigation::is_plain(key))
            {
                self.dialog = None;
            }
            return Vec::new();
        }
        if cancel {
            self.dialog = None;
            self.status = "Cancelled".into();
            return Vec::new();
        }
        let Some(mut dialog) = self.dialog.take() else {
            return Vec::new();
        };
        let (keep, actions) = match &mut dialog {
            Dialog::Help => (true, Vec::new()),
            Dialog::Prompt { kind, input, .. } => {
                if key.code == KeyCode::Enter {
                    let value = input.take();
                    (false, self.submit_prompt(kind.clone(), value))
                } else {
                    input.handle_key(key);
                    (true, Vec::new())
                }
            }
            Dialog::CreateRoom {
                name,
                password,
                field,
            } => match key.code {
                KeyCode::Tab | KeyCode::BackTab => {
                    *field = 1 - *field;
                    (true, Vec::new())
                }
                KeyCode::Enter if *field == 0 => {
                    *field = 1;
                    (true, Vec::new())
                }
                KeyCode::Enter => {
                    let name = name.value().trim().to_string();
                    if name.is_empty() {
                        self.status = "Room name cannot be empty".into();
                        (true, Vec::new())
                    } else {
                        let password = password.value().to_string();
                        self.busy = true;
                        self.status = "Creating room...".into();
                        (false, vec![Action::CreateRoom { name, password }])
                    }
                }
                _ => {
                    if *field == 0 {
                        name.handle_key(key);
                    } else {
                        password.handle_key(key);
                    }
                    (true, Vec::new())
                }
            },
            Dialog::Rooms { items, selected } => {
                if super::navigation::move_selection(selected, items.len(), key) {
                    (true, Vec::new())
                } else {
                    match key.code {
                        KeyCode::Enter => {
                            let Some(room) = items.get(*selected) else {
                                return Vec::new();
                            };
                            if room.membership_status.as_deref() == Some("active") {
                                self.busy = true;
                                self.status = format!("Connecting to {}...", room.name);
                                return vec![Action::ConnectRoom {
                                    room_id: room.id,
                                    password: self.room_passwords.get(&room.id).cloned(),
                                    target_message: None,
                                }];
                            }
                            if room.membership_status.as_deref() == Some("pending") {
                                self.status = "Join request is still waiting for approval".into();
                                return Vec::new();
                            }
                            if room.has_password {
                                self.dialog = Some(Dialog::Prompt {
                                    title: format!("Password for {}", room.name),
                                    kind: PromptKind::RoomJoinPassword(room.id),
                                    input: super::input::TextField::password(),
                                });
                                return Vec::new();
                            }
                            self.busy = true;
                            self.status = format!("Joining {}...", room.name);
                            (
                                false,
                                vec![Action::JoinRoom {
                                    room_id: room.id,
                                    password: None,
                                }],
                            )
                        }
                        _ => (true, Vec::new()),
                    }
                }
            }
            Dialog::FavoriteEditor {
                id,
                version,
                title,
                content,
                field,
            } => match key.code {
                KeyCode::Tab | KeyCode::BackTab => {
                    *field = 1 - *field;
                    (true, Vec::new())
                }
                KeyCode::Enter if *field == 0 => {
                    *field = 1;
                    (true, Vec::new())
                }
                KeyCode::Enter => {
                    let title = title.value().trim().to_string();
                    let content = content.value().trim().to_string();
                    if title.is_empty() && content.is_empty() {
                        self.status = "A favorite needs a title or content".into();
                        (true, Vec::new())
                    } else {
                        self.busy = true;
                        self.status = "Saving favorite...".into();
                        (
                            false,
                            vec![Action::SaveFavorite {
                                id: *id,
                                version: *version,
                                title,
                                content,
                            }],
                        )
                    }
                }
                _ => {
                    if *field == 0 {
                        title.handle_key(key);
                    } else {
                        content.handle_key(key);
                    }
                    (true, Vec::new())
                }
            },
            Dialog::Confirm { kind, .. } => match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') if super::navigation::is_plain(key) => {
                    (false, self.confirm_action(kind.clone()))
                }
                KeyCode::Enter => (false, self.confirm_action(kind.clone())),
                KeyCode::Char('n') | KeyCode::Char('N') if super::navigation::is_plain(key) => {
                    (false, Vec::new())
                }
                _ => (true, Vec::new()),
            },
        };
        if keep {
            self.dialog = Some(dialog);
        }
        actions
    }

    fn submit_prompt(&mut self, kind: PromptKind, value: String) -> Vec<Action> {
        match kind {
            PromptKind::RoomJoinPassword(room_id) => {
                self.room_passwords.insert(room_id, value.clone());
                vec![Action::JoinRoom {
                    room_id,
                    password: Some(value),
                }]
            }
            PromptKind::Upload => self.active_room.map_or_else(Vec::new, |room_id| {
                vec![Action::Upload {
                    room_id,
                    password: self.room_passwords.get(&room_id).cloned(),
                    path: PathBuf::from(value),
                }]
            }),
            PromptKind::Download(attachment) => vec![Action::Download {
                attachment,
                path: PathBuf::from(value),
            }],
            PromptKind::EditMessage(message_id) => {
                vec![Action::Chat(crate::client_chat::ChatCommand::Edit {
                    message_id,
                    content: value,
                })]
            }
            PromptKind::Reaction(message_id) => {
                vec![Action::Chat(crate::client_chat::ChatCommand::React {
                    message_id,
                    emoji: value,
                    active: true,
                })]
            }
        }
    }

    fn confirm_action(&mut self, kind: ConfirmKind) -> Vec<Action> {
        match kind {
            ConfirmKind::RecallMessage(id) => {
                vec![Action::Chat(crate::client_chat::ChatCommand::Recall(id))]
            }
            ConfirmKind::DeleteFavorite(id) => {
                self.busy = true;
                vec![Action::DeleteFavorite(id)]
            }
        }
    }
}
