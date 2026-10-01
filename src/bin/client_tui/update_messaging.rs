//! TG-1205 keys and answers: composer slash commands, quote replies, reaction removal, the
//! scheduled-messages and in-chat search dialogs, and joining by invite link.

use crossterm::event::{KeyCode, KeyEvent};
use reqwest::StatusCode;
use uuid::Uuid;

use crate::client_chat::ChatCommand;

use super::input::TextField;
use super::messaging::{parse_compose, ComposeCommand, MessagingAction, MessagingEvent};
use super::model::{Action, App, Dialog, Focus, PromptKind, View};

const MAX_MESSAGE_CHARS: usize = 4096;
/// The server's quote limit (`MAX_QUOTE_UTF16`).
const MAX_QUOTE_UTF16: usize = 1024;

impl App {
    /// Enter in the composer: a slash command or an ordinary (possibly quoting) message.
    pub(super) fn submit_compose(&mut self, text: String) -> Vec<Action> {
        let command = parse_compose(&text, chrono::Local::now());
        let content = match &command {
            ComposeCommand::Send { content, .. } | ComposeCommand::Schedule { content, .. } => {
                Some(content.clone())
            }
            _ => None,
        };
        if content.is_some() && self.chat.is_none() {
            self.status = "Chat connection is not ready; draft preserved".into();
            return Vec::new();
        }
        if content.is_some_and(|c| c.chars().count() > MAX_MESSAGE_CHARS) {
            self.status = format!("Message is longer than {MAX_MESSAGE_CHARS} characters");
            return Vec::new();
        }
        self.compose.clear();
        match command {
            ComposeCommand::Send { content, silent } => {
                let reply_to = self.reply_to.take();
                let quote = self.reply_quote.take();
                let mut command = self.queue_outgoing_message(content, reply_to);
                if let ChatCommand::Send {
                    silent: s,
                    reply_quote,
                    ..
                } = &mut command
                {
                    *s = silent;
                    *reply_quote = quote;
                }
                if silent {
                    self.status = "Sending silently...".into();
                }
                vec![Action::Chat(command)]
            }
            ComposeCommand::Schedule {
                content,
                at,
                silent,
            } => {
                let Some(room_id) = self.active_chat else {
                    self.compose = TextField::new(text.clone());
                    self.status = "Open a conversation first".into();
                    return Vec::new();
                };
                self.status = "Scheduling...".into();
                vec![Action::Messaging(MessagingAction::Schedule {
                    room_id,
                    content,
                    at,
                    silent,
                })]
            }
            ComposeCommand::ListScheduled => self.open_scheduled(),
            ComposeCommand::Search(query) => self.search_chat(query),
            ComposeCommand::Join(link) => self.join_invite_action(&link),
            ComposeCommand::Usage(usage) => {
                self.compose = TextField::new(text.clone());
                self.status = usage.into();
                Vec::new()
            }
        }
    }

    pub(super) fn open_scheduled(&mut self) -> Vec<Action> {
        let Some(room_id) = self.active_chat else {
            self.status = "Open a conversation first".into();
            return Vec::new();
        };
        vec![Action::Messaging(MessagingAction::LoadScheduled(room_id))]
    }

    pub(super) fn prompt_quote_selected(&mut self) {
        let Some(message) = self.selected_message().filter(|m| !m.recalled) else {
            return;
        };
        if message.content.trim().is_empty() {
            self.status = "Only text can be quoted".into();
            return;
        }
        self.dialog = Some(Dialog::Prompt {
            title: "Quote (trim to the part to quote)".into(),
            kind: PromptKind::Quote(message.id),
            input: TextField::new(message.content.clone()),
        });
    }

    pub(super) fn prompt_simple(&mut self, title: &str, kind: PromptKind) {
        self.dialog = Some(Dialog::Prompt {
            title: title.into(),
            kind,
            input: TextField::default(),
        });
    }

    pub(super) fn submit_messaging_prompt(
        &mut self,
        kind: PromptKind,
        value: String,
    ) -> Vec<Action> {
        match kind {
            PromptKind::Quote(message_id) => {
                let quote = value.trim();
                let original = self
                    .messages
                    .iter()
                    .find(|m| m.id == message_id)
                    .map(|m| m.content.as_str())
                    .unwrap_or_default();
                if quote.is_empty() || !original.contains(quote) {
                    self.status = "The quote must be part of the message".into();
                } else if quote.encode_utf16().count() > MAX_QUOTE_UTF16 {
                    self.status = "The quote is too long".into();
                } else {
                    self.reply_to = Some(message_id);
                    self.reply_quote = Some(quote.to_string());
                    self.focus = Focus::Input;
                }
                Vec::new()
            }
            PromptKind::ChatSearch => self.search_chat(value.trim().to_string()),
            PromptKind::JoinInvite => self.join_invite_action(&value),
            _ => unreachable!("not a messaging prompt"),
        }
    }

    /// `+` prompt: `👍` adds, `-👍` removes.
    pub(super) fn reaction_action(&mut self, message_id: Uuid, value: &str) -> Vec<Action> {
        let value = value.trim();
        let (emoji, active) = match value.strip_prefix('-') {
            Some(emoji) => (emoji.trim(), false),
            None => (value, true),
        };
        if emoji.is_empty() {
            return Vec::new();
        }
        vec![Action::Chat(ChatCommand::React {
            message_id,
            emoji: emoji.to_string(),
            active,
        })]
    }

    fn search_chat(&mut self, query: String) -> Vec<Action> {
        let Some(room_id) = self.active_chat else {
            self.status = "Open a conversation first".into();
            return Vec::new();
        };
        if query.is_empty() || query.chars().count() > 200 {
            self.status = "Search text must be 1–200 characters".into();
            return Vec::new();
        }
        self.status = format!("Searching for “{query}”...");
        vec![Action::Messaging(MessagingAction::Search {
            room_id,
            password: self.chat_passwords.get(&room_id).cloned(),
            query,
        })]
    }

    fn join_invite_action(&mut self, link: &str) -> Vec<Action> {
        match crate::client_api_messages::invite_token(link) {
            Some(token) => {
                self.busy = true;
                self.status = "Joining...".into();
                vec![Action::Messaging(MessagingAction::JoinInvite(token))]
            }
            None => {
                self.status = "That is not an invite link".into();
                Vec::new()
            }
        }
    }

    /// Keys of the two TG-1205 list dialogs; returns whether the dialog stays open.
    pub(super) fn messaging_dialog_key(
        &mut self,
        key: KeyEvent,
        dialog: &mut Dialog,
    ) -> (bool, Vec<Action>) {
        match dialog {
            Dialog::Scheduled {
                room_id,
                items,
                selected,
            } => {
                if super::navigation::move_selection(selected, items.len(), key) {
                    return (true, Vec::new());
                }
                let Some(item) = items.get(*selected) else {
                    return (true, Vec::new());
                };
                let (room_id, id) = (*room_id, item.id);
                match key.code {
                    KeyCode::Enter => (
                        true,
                        vec![Action::Messaging(MessagingAction::SendScheduledNow {
                            room_id,
                            id,
                        })],
                    ),
                    KeyCode::Char('x') | KeyCode::Delete => (
                        true,
                        vec![Action::Messaging(MessagingAction::CancelScheduled {
                            room_id,
                            id,
                        })],
                    ),
                    _ => (true, Vec::new()),
                }
            }
            Dialog::ChatSearch {
                items, selected, ..
            } => {
                if super::navigation::move_selection(selected, items.len(), key) {
                    return (true, Vec::new());
                }
                if key.code != KeyCode::Enter {
                    return (true, Vec::new());
                }
                let Some(id) = items.get(*selected).map(|item| item.id) else {
                    return (false, Vec::new());
                };
                (false, self.jump_to_message(id))
            }
            _ => (true, Vec::new()),
        }
    }

    fn jump_to_message(&mut self, id: Uuid) -> Vec<Action> {
        if let Some(index) = self.messages.iter().position(|m| m.id == id) {
            self.message_index = index;
            self.focus = Focus::Content;
            return Vec::new();
        }
        match self.active_chat {
            Some(room_id) => self.connect_chat(room_id, Some(id)),
            None => Vec::new(),
        }
    }

    pub(super) fn apply_messaging_event(&mut self, event: MessagingEvent) -> Vec<Action> {
        self.busy = false;
        match event {
            MessagingEvent::Scheduled {
                room_id,
                result: Ok(items),
            } => {
                let selected = match &self.dialog {
                    Some(Dialog::Scheduled { selected, .. }) => *selected,
                    _ => 0,
                };
                if items.is_empty() {
                    self.status = "No scheduled messages in this chat".into();
                }
                self.dialog = Some(Dialog::Scheduled {
                    room_id,
                    selected: selected.min(items.len().saturating_sub(1)),
                    items,
                });
            }
            MessagingEvent::ScheduledChanged {
                room_id,
                result: Ok(status),
            } => {
                self.status = status;
                if matches!(self.dialog, Some(Dialog::Scheduled { .. })) {
                    return vec![Action::Messaging(MessagingAction::LoadScheduled(room_id))];
                }
            }
            MessagingEvent::Found {
                query,
                result: Ok(items),
            } => {
                self.status = format!("{} result(s) for “{query}”", items.len());
                self.dialog = Some(Dialog::ChatSearch {
                    query,
                    items,
                    selected: 0,
                });
            }
            MessagingEvent::Joined(Ok(joined)) => match joined.chat_id {
                Some(room_id) if joined.status == "active" => {
                    self.view = View::Chats;
                    self.status = "Joined".into();
                    let mut actions = vec![Action::LoadConversations];
                    actions.extend(self.connect_chat(room_id, None));
                    return actions;
                }
                _ => self.status = "Join request sent; an admin must approve it".into(),
            },
            MessagingEvent::Joined(Err(error)) => {
                self.status = match error.status {
                    Some(StatusCode::NOT_FOUND) => "Invite link not found".into(),
                    Some(StatusCode::GONE) => "Invite link expired, revoked or full".into(),
                    Some(StatusCode::FORBIDDEN) => "You are banned from this chat".into(),
                    Some(StatusCode::LOCKED) => "This chat is locked".into(),
                    _ => error.to_string(),
                };
            }
            MessagingEvent::Scheduled {
                result: Err(error), ..
            }
            | MessagingEvent::ScheduledChanged {
                result: Err(error), ..
            }
            | MessagingEvent::Found {
                result: Err(error), ..
            } => self.status = error.to_string(),
        }
        Vec::new()
    }
}
