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
                    // TG-1103: the list shows the current folder's share of all chats.
                    self.social.all_conversations = items;
                    self.conversations = self.social.visible_conversations();
                    self.sync_conversation_selection();
                    self.status = format!("{} conversations", self.conversations.len());
                    Vec::new()
                }
                Err(error) => self.api_error(error),
            },
            AppEvent::Chats(result) => {
                self.busy = false;
                match result {
                    Ok(items) => self.dialog = Some(Dialog::Chats { items, selected: 0 }),
                    Err(error) => self.status = error.to_string(),
                }
                Vec::new()
            }
            AppEvent::ChatCreated { password, result } => match result {
                Ok(chat) => {
                    self.busy = false;
                    if let Some(password) = password.clone() {
                        self.chat_passwords.insert(chat.id, password);
                    }
                    vec![
                        Action::LoadConversations,
                        Action::ConnectChat {
                            room_id: chat.id,
                            password,
                            target_message: None,
                        },
                    ]
                }
                Err(error) => self.api_error(error),
            },
            AppEvent::ChatJoined {
                room_id,
                password,
                result,
            } => match result {
                Ok(membership) if membership.status == "active" => {
                    vec![
                        Action::LoadConversations,
                        Action::ConnectChat {
                            room_id,
                            password,
                            target_message: None,
                        },
                    ]
                }
                Ok(_) => {
                    self.busy = false;
                    self.status = "Join request submitted; waiting for chat approval".into();
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
                        self.active_chat = Some(room_id);
                        self.active_room_name = name;
                        self.sync_conversation_selection();
                        self.messages.clear();
                        self.message_index = 0;
                        self.pending_message = target_message;
                        self.chat = Some(sender);
                        self.view = super::model::View::Chats;
                        self.focus = super::model::Focus::Content;
                        self.status = "Loading message history...".into();
                        // TG-907: the pinned line under the chat header.
                        self.social.pins.clear();
                        return vec![Action::Social(super::social::SocialAction::LoadPins(
                            room_id,
                        ))];
                    }
                    Err(error) => self.status = error,
                }
                Vec::new()
            }
            AppEvent::Chat { room_id, event } => self.apply_chat_event(room_id, event),
            AppEvent::Social(event) => self.apply_social_event(event),
            AppEvent::Messaging(event) => self.apply_messaging_event(event),
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
        if let Some(index) = self.active_chat.and_then(|active_chat| {
            self.conversations
                .iter()
                .position(|conversation| conversation.room_id == active_chat)
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
        client_api::{ChatMembership, Conversation, ConversationPreferences},
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

        let actions = app.apply_event(AppEvent::ChatJoined {
            room_id: Uuid::new_v4(),
            password: None,
            result: Ok(ChatMembership {
                status: "pending".into(),
            }),
        });

        assert!(actions.is_empty());
        assert!(!app.busy);
        assert!(app.status.contains("waiting for chat approval"));
    }

    #[test]
    fn conversations_loaded_after_connect_select_the_active_chat() {
        let mut app = App::new("http://localhost".into(), UserConfig::default(), None);
        let first_chat = Uuid::new_v4();
        let active_chat = Uuid::new_v4();
        let (sender, _receiver) = mpsc::unbounded_channel();

        app.apply_event(AppEvent::ChatConnected {
            room_id: active_chat,
            target_message: None,
            result: Ok(("Active chat".into(), sender)),
        });
        app.apply_event(AppEvent::Conversations(Ok(vec![
            conversation(first_chat, "First chat"),
            conversation(active_chat, "Active chat"),
        ])));

        assert_eq!(app.conversation_index, 1);
    }
}
