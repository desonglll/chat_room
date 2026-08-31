//! Reduction of background HTTP and WebSocket results into application state.

use super::model::{Action, App, AppEvent, Dialog};

impl App {
    pub fn apply_event(&mut self, event: AppEvent) -> Vec<Action> {
        match event {
            AppEvent::SessionValidated(result) => self.session_validated(result),
            AppEvent::Authenticated(result) => self.authenticated(result),
            AppEvent::LoggedOut(result) => {
                self.clear_session();
                self.status =
                    result.map_or_else(|error| error.to_string(), |_| "Logged out".into());
                Vec::new()
            }
            AppEvent::Conversations(result) => match result {
                Ok(items) => {
                    self.busy = false;
                    self.conversations = items;
                    self.sync_conversation_selection();
                    self.status = format!("{} conversations", self.conversations.len());
                    Vec::new()
                }
                Err(error) => self.api_error(error),
            },
            AppEvent::Rooms(result) => {
                self.busy = false;
                match result {
                    Ok(items) => self.dialog = Some(Dialog::Rooms { items, selected: 0 }),
                    Err(error) => self.status = error.to_string(),
                }
                Vec::new()
            }
            AppEvent::RoomCreated { password, result } => match result {
                Ok(room) => {
                    self.busy = false;
                    if let Some(password) = password.clone() {
                        self.room_passwords.insert(room.id, password);
                    }
                    vec![
                        Action::LoadConversations,
                        Action::ConnectRoom {
                            room_id: room.id,
                            password,
                            target_message: None,
                        },
                    ]
                }
                Err(error) => self.api_error(error),
            },
            AppEvent::RoomJoined {
                room_id,
                password,
                result,
            } => match result {
                Ok(membership) if membership.status == "active" => {
                    vec![
                        Action::LoadConversations,
                        Action::ConnectRoom {
                            room_id,
                            password,
                            target_message: None,
                        },
                    ]
                }
                Ok(_) => {
                    self.busy = false;
                    self.status = "Join request submitted; waiting for room approval".into();
                    Vec::new()
                }
                Err(error) => self.api_error(error),
            },
            AppEvent::ChatConnected {
                room_id,
                target_message,
                result,
            } => {
                self.busy = false;
                match result {
                    Ok((name, sender)) => {
                        self.active_room = Some(room_id);
                        self.active_room_name = name;
                        self.sync_conversation_selection();
                        self.messages.clear();
                        self.message_index = 0;
                        self.pending_message = target_message;
                        self.chat = Some(sender);
                        self.view = super::model::View::Chats;
                        self.focus = super::model::Focus::Content;
                        self.status = "Loading message history...".into();
                    }
                    Err(error) => self.status = error,
                }
                Vec::new()
            }
            AppEvent::Chat { room_id, event } => self.apply_chat_event(room_id, event),
            AppEvent::Uploaded(result) => {
                self.busy = false;
                self.status = result
                    .map_or_else(|error| error, |file| format!("Uploaded {}", file.file_name));
                Vec::new()
            }
            AppEvent::Downloaded(result) => {
                self.busy = false;
                self.status = result.map_or_else(
                    |error| error,
                    |path| format!("Downloaded to {}", path.display()),
                );
                Vec::new()
            }
            AppEvent::PreferencesUpdated(result) => match result {
                Ok(_) => vec![Action::LoadConversations],
                Err(error) => self.api_error(error),
            },
            AppEvent::Search(result) => {
                self.busy = false;
                match result {
                    Ok(page) => {
                        self.search_results = page.items;
                        self.search_index = 0;
                        self.focus = super::model::Focus::List;
                        self.status = format!("{} search results", self.search_results.len());
                    }
                    Err(error) => self.status = error.to_string(),
                }
                Vec::new()
            }
            AppEvent::Notifications(result) => {
                self.busy = false;
                match result {
                    Ok(page) => {
                        self.notifications = page.items;
                        self.notification_index = self
                            .notification_index
                            .min(self.notifications.len().saturating_sub(1));
                        self.status = format!("{} notifications", self.notifications.len());
                    }
                    Err(error) => self.status = error.to_string(),
                }
                Vec::new()
            }
            AppEvent::NotificationRead(result) => match result {
                Ok(()) => vec![Action::LoadNotifications],
                Err(error) => self.api_error(error),
            },
            AppEvent::Favorites(result) => {
                self.busy = false;
                match result {
                    Ok(items) => {
                        self.favorites = items;
                        self.favorite_index = self
                            .favorite_index
                            .min(self.favorites.len().saturating_sub(1));
                        self.status = format!("{} favorites", self.favorites.len());
                    }
                    Err(error) => self.status = error.to_string(),
                }
                Vec::new()
            }
            AppEvent::FavoriteSaved(result) => match result {
                Ok(_) => vec![Action::LoadFavorites],
                Err(error) => self.api_error(error),
            },
            AppEvent::MessageFavorited(result) => {
                self.status =
                    result.map_or_else(|error| error.to_string(), |_| "Message saved".into());
                Vec::new()
            }
            AppEvent::FavoriteDeleted(result) => match result {
                Ok(()) => vec![Action::LoadFavorites],
                Err(error) => self.api_error(error),
            },
            AppEvent::AiThreads(result) => {
                self.busy = false;
                match result {
                    Ok(items) => {
                        self.ai_threads = items;
                        self.ai_thread_index = self
                            .ai_thread_index
                            .min(self.ai_threads.len().saturating_sub(1));
                        self.status = format!("{} AI threads", self.ai_threads.len());
                        if let Some(thread) = self.selected_ai_thread() {
                            return vec![Action::LoadAiMessages(thread.id)];
                        }
                    }
                    Err(error) => self.status = error.to_string(),
                }
                Vec::new()
            }
            AppEvent::AiMessages { thread_id, result } => {
                if self
                    .selected_ai_thread()
                    .is_some_and(|thread| thread.id == thread_id)
                {
                    match result {
                        Ok(items) => {
                            self.ai_messages = items;
                            self.ai_message_index = self.ai_messages.len().saturating_sub(1);
                        }
                        Err(error) => self.status = error.to_string(),
                    }
                }
                Vec::new()
            }
            AppEvent::AiRunStarted(result) => {
                match result {
                    Ok((thread, run)) => {
                        if !self.ai_threads.iter().any(|item| item.id == thread.id) {
                            self.ai_threads.insert(0, thread);
                            self.ai_thread_index = 0;
                        }
                        self.status = format!("AI run {}", run.status);
                    }
                    Err(error) => {
                        self.ai_running = false;
                        self.status = error.to_string();
                    }
                }
                Vec::new()
            }
            AppEvent::AiRunPolled(result) => {
                match result {
                    Ok(run) => {
                        self.status = run
                            .error_message
                            .unwrap_or_else(|| format!("AI run {}", run.status));
                        self.ai_running = !matches!(run.status.as_str(), "completed" | "failed");
                    }
                    Err(error) => {
                        self.ai_running = false;
                        self.status = error.to_string();
                    }
                }
                Vec::new()
            }
        }
    }

    fn sync_conversation_selection(&mut self) {
        if let Some(index) = self.active_room.and_then(|active_room| {
            self.conversations
                .iter()
                .position(|conversation| conversation.room_id == active_room)
        }) {
            self.conversation_index = index;
        } else {
            self.conversation_index = self
                .conversation_index
                .min(self.conversations.len().saturating_sub(1));
        }
    }
}

#[cfg(test)]
mod tests {
    use tokio::sync::mpsc;
    use uuid::Uuid;

    use super::*;
    use crate::{
        client_api::{Conversation, ConversationPreferences, RoomMembership},
        client_auth::UserConfig,
    };

    fn conversation(room_id: Uuid, title: &str) -> Conversation {
        Conversation {
            room_id,
            kind: "group".into(),
            title: title.into(),
            unread_count: 0,
            group: None,
            preferences: ConversationPreferences::default(),
            last_message: None,
        }
    }

    #[test]
    fn pending_membership_does_not_open_a_chat_connection() {
        let mut app = App::new("http://localhost".into(), UserConfig::default(), None);
        app.busy = true;

        let actions = app.apply_event(AppEvent::RoomJoined {
            room_id: Uuid::new_v4(),
            password: None,
            result: Ok(RoomMembership {
                status: "pending".into(),
            }),
        });

        assert!(actions.is_empty());
        assert!(!app.busy);
        assert!(app.status.contains("waiting for room approval"));
    }

    #[test]
    fn conversations_loaded_after_connect_select_the_active_room() {
        let mut app = App::new("http://localhost".into(), UserConfig::default(), None);
        let first_room = Uuid::new_v4();
        let active_room = Uuid::new_v4();
        let (sender, _receiver) = mpsc::unbounded_channel();

        app.apply_event(AppEvent::ChatConnected {
            room_id: active_room,
            target_message: None,
            result: Ok(("Active room".into(), sender)),
        });
        app.apply_event(AppEvent::Conversations(Ok(vec![
            conversation(first_room, "First room"),
            conversation(active_room, "Active room"),
        ])));

        assert_eq!(app.conversation_index, 1);
    }
}
