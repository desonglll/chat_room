//! TG-907: the Contacts tab, pin/unpin and forward key flows and their answers.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use uuid::Uuid;

use super::{
    model::{App, AppEvent, Dialog, Focus, PromptKind, Screen, View},
    social::{ContactsData, SocialAction, SocialEvent},
    Action,
};
use crate::{
    client_api::{Conversation, ConversationPreferences},
    client_api_social::{Friend, FriendRequest, RequestUser},
    client_auth::UserConfig,
    client_chat::{ChatMessage, DeliveryState},
};

fn app() -> App {
    let mut app = App::new("http://localhost".into(), UserConfig::default(), None);
    app.screen = Screen::Main;
    app.username = "alice".into();
    app
}

fn press(app: &mut App, code: KeyCode) -> Vec<Action> {
    app.handle_key(KeyEvent::new(code, KeyModifiers::NONE))
}

fn press_shift(app: &mut App, code: KeyCode) -> Vec<Action> {
    app.handle_key(KeyEvent::new(code, KeyModifiers::SHIFT))
}

fn contacts_app() -> (App, Uuid, Uuid) {
    let (friend_id, requester) = (Uuid::from_u128(1), Uuid::from_u128(2));
    let mut app = app();
    app.view = View::Contacts;
    app.focus = Focus::List;
    app.apply_event(AppEvent::Social(SocialEvent::Contacts(Ok(ContactsData {
        friends: vec![Friend {
            id: friend_id,
            username: "bob".into(),
            display_name: "Bob".into(),
            remark: String::new(),
        }],
        requests: vec![FriendRequest {
            user: RequestUser {
                id: requester,
                username: "eve".into(),
                display_name: String::new(),
            },
        }],
        ..ContactsData::default()
    }))));
    (app, friend_id, requester)
}

#[test]
fn a_request_is_accepted_or_declined_from_the_contacts_tab() {
    let (mut app, _, requester) = contacts_app();
    assert!(app.status.contains("1 contacts, 1 request"));
    let actions = press(&mut app, KeyCode::Char('a'));
    assert!(matches!(
        actions.as_slice(),
        [Action::Social(SocialAction::Respond { user_id, accept: true })] if *user_id == requester
    ));
    let actions = press(&mut app, KeyCode::Char('x'));
    assert!(matches!(
        actions.as_slice(),
        [Action::Social(SocialAction::Respond { accept: false, .. })]
    ));
}

#[test]
fn enter_on_a_friend_opens_the_private_chat_and_connects() {
    let (mut app, friend_id, _) = contacts_app();
    press(&mut app, KeyCode::Down);
    let actions = press(&mut app, KeyCode::Enter);
    assert!(matches!(
        actions.as_slice(),
        [Action::Social(SocialAction::OpenDirectChat(id))] if *id == friend_id
    ));
    let room = Uuid::from_u128(7);
    let actions = app.apply_event(AppEvent::Social(SocialEvent::DirectChatOpened(Ok(room))));
    assert_eq!(app.view, View::Chats);
    assert!(matches!(actions.first(), Some(Action::LoadConversations)));
    assert!(actions
        .iter()
        .any(|action| matches!(action, Action::ConnectChat { room_id, .. } if *room_id == room)));
}

#[test]
fn n_prompts_for_a_username_and_a_change_reloads_contacts() {
    let (mut app, _, _) = contacts_app();
    press(&mut app, KeyCode::Char('n'));
    assert!(matches!(
        app.dialog,
        Some(Dialog::Prompt {
            kind: PromptKind::AddContact,
            ..
        })
    ));
    let actions = app.apply_event(AppEvent::Social(SocialEvent::ContactsChanged(Ok(
        "Request sent to @dan".into(),
    ))));
    assert_eq!(app.status, "Request sent to @dan");
    assert!(matches!(
        actions.as_slice(),
        [Action::Social(SocialAction::LoadContacts)]
    ));
}

fn chat_app() -> (App, Uuid, Uuid) {
    let (room, message) = (Uuid::from_u128(10), Uuid::from_u128(11));
    let mut app = app();
    app.active_chat = Some(room);
    app.focus = Focus::Content;
    app.messages.push(ChatMessage {
        id: message,
        client_message_id: None,
        sender: "bob".into(),
        content: "pin me".into(),
        attachment: None,
        timestamp: "2026-10-01T09:00:00Z".into(),
        recalled: false,
        edited: false,
        delivery: DeliveryState::Sent,
    });
    (app, room, message)
}

#[test]
fn shift_p_pins_then_unpins_the_selected_message() {
    let (mut app, room, message) = chat_app();
    let actions = press_shift(&mut app, KeyCode::Char('P'));
    assert!(matches!(
        actions.as_slice(),
        [Action::Social(SocialAction::SetPinned { room_id, message_id, pinned: true })]
            if *room_id == room && *message_id == message
    ));
    app.social.pins = vec![(message, "pin me".into())];
    let actions = press_shift(&mut app, KeyCode::Char('P'));
    assert!(matches!(
        actions.as_slice(),
        [Action::Social(SocialAction::SetPinned {
            pinned: false,
            ..
        })]
    ));
}

#[test]
fn a_pins_changed_frame_reloads_the_pins_of_the_open_chat() {
    let (mut app, room, _) = chat_app();
    let actions = app.apply_event(AppEvent::Chat {
        room_id: room,
        event: crate::client_chat::ChatEvent::PinsChanged,
    });
    assert!(
        matches!(actions.as_slice(), [Action::Social(SocialAction::LoadPins(id))] if *id == room)
    );
}

#[test]
fn forward_matches_a_chat_by_title_and_reports_privacy_refusals() {
    let (mut app, _, message) = chat_app();
    app.conversations.push(Conversation {
        room_id: Uuid::from_u128(20),
        kind: "direct".into(),
        title: "Bob".into(),
        unread_count: 0,
        group: None,
        preferences: ConversationPreferences::default(),
        last_message: None,
    });
    press_shift(&mut app, KeyCode::Char('F'));
    assert!(
        matches!(app.dialog, Some(Dialog::Prompt { kind: PromptKind::Forward(id), .. }) if id == message)
    );
    let actions = app.forward_action(message, "bo");
    assert!(matches!(
        actions.as_slice(),
        [Action::Social(SocialAction::Forward { target, .. })] if *target == Uuid::from_u128(20)
    ));
    assert!(app.forward_action(message, "nobody").is_empty());
    assert!(app.status.contains("No chat named"));
    app.apply_event(AppEvent::Social(SocialEvent::Forwarded {
        title: "Bob".into(),
        result: Ok(crate::client_api_social::ForwardResult {
            forwarded_message_id: None,
            skipped_reason: Some("voice_messages_restricted".into()),
        }),
    }));
    assert!(app.status.contains("does not accept voice messages"));
}

fn screen(app: &mut App) -> String {
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(100, 24)).unwrap();
    terminal
        .draw(|frame| super::render::render(frame, app))
        .unwrap();
    terminal.backend().to_string()
}

#[test]
fn the_contacts_tab_and_the_pinned_title_are_drawn() {
    let (mut app, friend_id, _) = contacts_app();
    app.social
        .contacts
        .statuses
        .insert(friend_id, serde_json::json!({ "kind": "online" }));
    let drawn = screen(&mut app);
    assert!(drawn.contains("Contacts"), "{drawn}");
    assert!(drawn.contains("Request"), "{drawn}");
    assert!(drawn.contains("@eve"), "{drawn}");
    assert!(drawn.contains("Bob"), "{drawn}");
    assert!(drawn.contains("online"), "{drawn}");

    let (mut chat, _, message) = chat_app();
    chat.active_room_name = "Team".into();
    chat.social.pins = vec![(message, "pin me".into())];
    let drawn = screen(&mut chat);
    assert!(
        drawn.contains("pin me (") || drawn.contains("📌 pin me"),
        "{drawn}"
    );
    assert!(drawn.contains("pinned"), "{drawn}");
}
