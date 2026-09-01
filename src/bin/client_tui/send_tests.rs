use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tokio::sync::mpsc;
use uuid::Uuid;

use super::{
    model::{App, Focus, Screen},
    Action,
};
use crate::{
    client_api::{Conversation, ConversationGroup, ConversationPreferences},
    client_auth::UserConfig,
    client_chat::{ChatCommand, ChatEvent, ChatMessage, DeliveryState},
};

fn app() -> App {
    let mut app = App::new("http://localhost".into(), UserConfig::default(), None);
    app.screen = Screen::Main;
    app.username = "alice".into();
    app
}

fn conversation(room_id: Uuid, has_password: bool) -> Conversation {
    Conversation {
        room_id,
        kind: "group".into(),
        title: "Project room".into(),
        unread_count: 0,
        group: Some(ConversationGroup { has_password }),
        preferences: ConversationPreferences::default(),
        last_message: None,
    }
}

fn key(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
    KeyEvent::new(code, modifiers)
}

#[test]
fn disconnected_send_keeps_the_draft() {
    let mut app = app();
    app.active_room = Some(Uuid::new_v4());
    app.focus = Focus::Input;
    app.compose.set("do not lose this");

    let actions = app.handle_key(key(KeyCode::Enter, KeyModifiers::NONE));

    assert!(actions.is_empty());
    assert_eq!(app.compose.value(), "do not lose this");
    assert!(app.status.contains("connection"));
}

#[test]
fn connected_send_appears_immediately_and_reaches_the_chat_channel() {
    let room_id = Uuid::new_v4();
    let (chat, mut commands) = mpsc::unbounded_channel();
    let (events, _event_receiver) = mpsc::unbounded_channel();
    let mut app = app();
    app.active_room = Some(room_id);
    app.active_room_name = "Project room".into();
    app.chat = Some(chat);
    app.focus = Focus::Input;
    app.compose.set("ship it");

    let actions = app.handle_key(key(KeyCode::Enter, KeyModifiers::NONE));
    let client_message_id = match actions.as_slice() {
        [Action::Chat(ChatCommand::Send {
            content,
            client_message_id,
            ..
        })] if content == "ship it" => *client_message_id,
        _ => panic!("expected one send command"),
    };
    assert_eq!(app.messages.len(), 1);
    assert_eq!(app.messages[0].content, "ship it");
    assert_eq!(app.messages[0].delivery, DeliveryState::Sending);
    assert!(app.status.contains("Sending"));

    super::dispatch_actions(&mut app, actions, events);
    assert!(matches!(
        commands.try_recv(),
        Ok(ChatCommand::Send { content, .. }) if content == "ship it"
    ));

    let server_message_id = Uuid::new_v4();
    let echoed = ChatMessage {
        id: server_message_id,
        client_message_id: Some(client_message_id),
        sender: "alice".into(),
        content: "ship it".into(),
        attachment: None,
        timestamp: "2026-08-31T12:00:00Z".into(),
        recalled: false,
        edited: false,
        delivery: DeliveryState::Sent,
    };
    app.apply_chat_event(room_id, ChatEvent::Message(echoed.clone()));
    app.apply_chat_event(room_id, ChatEvent::Message(echoed));
    assert_eq!(app.messages.len(), 1);
    assert_eq!(app.messages[0].id, server_message_id);
    assert_eq!(app.messages[0].delivery, DeliveryState::Sent);
    assert_eq!(app.status, "Message sent");
}

#[test]
fn an_existing_private_conversation_opens_without_a_password_prompt() {
    let room_id = Uuid::new_v4();
    let mut app = app();
    app.conversations = vec![conversation(room_id, true)];

    let actions = app.handle_key(key(KeyCode::Enter, KeyModifiers::NONE));

    assert!(app.dialog.is_none());
    assert!(matches!(
        actions.as_slice(),
        [Action::ConnectRoom { room_id: selected, password: None, .. }] if *selected == room_id
    ));
}

#[test]
fn emacs_control_keys_move_through_conversations() {
    let first = Uuid::new_v4();
    let second = Uuid::new_v4();
    let mut app = app();
    app.conversations = vec![conversation(first, false), conversation(second, false)];

    app.handle_key(key(KeyCode::Char('n'), KeyModifiers::CONTROL));
    assert_eq!(app.conversation_index, 1);
    app.handle_key(key(KeyCode::Char('p'), KeyModifiers::CONTROL));
    assert_eq!(app.conversation_index, 0);
}

#[test]
fn closed_chat_channel_marks_the_pending_message_as_failed() {
    let room_id = Uuid::new_v4();
    let (chat, commands) = mpsc::unbounded_channel();
    drop(commands);
    let (events, _event_receiver) = mpsc::unbounded_channel();
    let mut app = app();
    app.active_room = Some(room_id);
    app.chat = Some(chat);
    app.focus = Focus::Input;
    app.compose.set("keep failure visible");

    let actions = app.handle_key(key(KeyCode::Enter, KeyModifiers::NONE));
    super::dispatch_actions(&mut app, actions, events);

    assert_eq!(app.messages.len(), 1);
    assert_eq!(app.messages[0].delivery, DeliveryState::Failed);
    assert!(app.status.contains("failed"));
}

#[test]
fn emacs_prefixes_do_not_trigger_plain_character_commands() {
    let room_id = Uuid::new_v4();
    let mut app = app();
    app.conversations = vec![conversation(room_id, false)];

    let archive = app.handle_key(key(KeyCode::Char('a'), KeyModifiers::CONTROL));
    let quit = app.handle_key(key(KeyCode::Char('q'), KeyModifiers::CONTROL));

    assert!(archive.is_empty());
    assert!(quit.is_empty());
    assert!(!app.conversations[0].preferences.is_archived);
}
