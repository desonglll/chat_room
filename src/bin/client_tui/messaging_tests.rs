//! TG-1205: slash commands, schedule times, and the quote / silent / scheduled / search / invite
//! key flows and their answers.

use chrono::{Local, TimeZone, Timelike};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use uuid::Uuid;

use super::*;
use crate::client_api_messages::{FoundMessage, InviteJoin, ScheduledMessage};
use crate::client_auth::UserConfig;
use crate::client_chat::{ChatCommand, ChatEvent, ChatMessage, DeliveryState};
use crate::client_tui::model::{Action, App, AppEvent, Dialog, Focus, PromptKind, Screen};

fn now() -> DateTime<Local> {
    Local.with_ymd_and_hms(2026, 10, 1, 12, 0, 0).unwrap()
}

#[test]
fn slash_commands_parse_and_ordinary_text_passes_through() {
    let send = |content: &str, silent| ComposeCommand::Send {
        content: content.into(),
        silent,
    };
    assert_eq!(parse_compose("hello", now()), send("hello", false));
    assert_eq!(parse_compose("/silent  hush ", now()), send("hush", true));
    assert_eq!(parse_compose("/shrug", now()), send("/shrug", false));
    assert_eq!(parse_compose("//silent x", now()), send("/silent x", false));
    assert_eq!(
        parse_compose("/scheduled", now()),
        ComposeCommand::ListScheduled
    );
    assert_eq!(
        parse_compose("/search  lunch", now()),
        ComposeCommand::Search("lunch".into())
    );
    assert_eq!(
        parse_compose("/join https://x/joinchat/t", now()),
        ComposeCommand::Join("https://x/joinchat/t".into())
    );
    for usage in [
        "/silent",
        "/search",
        "/join",
        "/schedule",
        "/schedule 2h",
        "/schedule soon hi",
    ] {
        assert!(
            matches!(parse_compose(usage, now()), ComposeCommand::Usage(_)),
            "{usage}"
        );
    }
    let ComposeCommand::Schedule {
        content,
        at,
        silent,
    } = parse_compose("/schedule 2h /silent stand up", now())
    else {
        panic!("expected a schedule");
    };
    assert_eq!((content.as_str(), silent), ("stand up", true));
    assert_eq!(
        at,
        (now() + chrono::Duration::hours(2)).with_timezone(&chrono::Utc)
    );
    let ComposeCommand::Schedule { at, content, .. } =
        parse_compose("/schedule 2026-10-03 09:30 board meeting", now())
    else {
        panic!("expected a dated schedule");
    };
    assert_eq!(content, "board meeting");
    assert_eq!(at.with_timezone(&Local).hour(), 9);
    assert_eq!(
        parse_compose("/schedule 2026-09-01 09:30 too late", now()),
        ComposeCommand::Usage("That time has already passed")
    );
}

#[test]
fn schedule_times_resolve_relative_clock_and_dates() {
    let utc = |t: DateTime<Local>| t.with_timezone(&chrono::Utc);
    assert_eq!(
        when("30m", now()),
        Some(utc(now() + chrono::Duration::minutes(30)))
    );
    assert_eq!(
        when("1d", now()),
        Some(utc(now() + chrono::Duration::days(1)))
    );
    assert_eq!(
        when("13:15", now()),
        Some(utc(Local.with_ymd_and_hms(2026, 10, 1, 13, 15, 0).unwrap()))
    );
    // Already past today → tomorrow.
    assert_eq!(
        when("08:00", now()),
        Some(utc(Local.with_ymd_and_hms(2026, 10, 2, 8, 0, 0).unwrap()))
    );
    for bad in ["0m", "-5m", "xh", "25:00", "tomorrow", ""] {
        assert_eq!(when(bad, now()), None, "{bad}");
    }
}

fn chat_app() -> (
    App,
    Uuid,
    Uuid,
    tokio::sync::mpsc::UnboundedReceiver<ChatCommand>,
) {
    let (room, message) = (Uuid::from_u128(20), Uuid::from_u128(21));
    let mut app = App::new("http://localhost".into(), UserConfig::default(), None);
    app.screen = Screen::Main;
    app.username = "alice".into();
    app.active_chat = Some(room);
    let (sender, commands) = tokio::sync::mpsc::unbounded_channel();
    app.chat = Some(sender);
    app.focus = Focus::Content;
    app.messages.push(ChatMessage {
        id: message,
        client_message_id: None,
        sender: "bob".into(),
        content: "are we shipping on friday".into(),
        attachment: None,
        timestamp: "2026-10-01T09:00:00Z".into(),
        recalled: false,
        edited: false,
        delivery: DeliveryState::Sent,
        media: Default::default(),
        extras: Default::default(),
    });
    (app, room, message, commands)
}

fn press(app: &mut App, code: KeyCode) -> Vec<Action> {
    app.handle_key(KeyEvent::new(code, KeyModifiers::NONE))
}

fn type_and_enter(app: &mut App, text: &str) -> Vec<Action> {
    match app.dialog.as_mut() {
        Some(Dialog::Prompt { input, .. }) => input.set(text),
        _ => app.compose.set(text),
    }
    press(app, KeyCode::Enter)
}

#[test]
fn quote_reply_sends_the_quoted_part_and_esc_drops_it() {
    let (mut app, _, message, _) = chat_app();
    assert!(app
        .handle_key(KeyEvent::new(KeyCode::Char('Q'), KeyModifiers::SHIFT))
        .is_empty());
    assert!(
        matches!(app.dialog, Some(Dialog::Prompt { kind: PromptKind::Quote(id), .. }) if id == message)
    );
    type_and_enter(&mut app, "not in there");
    assert_eq!(app.status, "The quote must be part of the message");
    assert!(app.reply_to.is_none());

    press(&mut app, KeyCode::Char('Q'));
    type_and_enter(&mut app, "shipping on friday");
    assert_eq!((app.reply_to, app.focus), (Some(message), Focus::Input));
    let actions = type_and_enter(&mut app, "yes");
    let [Action::Chat(ChatCommand::Send {
        reply_to,
        reply_quote,
        silent,
        ..
    })] = actions.as_slice()
    else {
        panic!("expected a send: {actions:?}");
    };
    assert_eq!(*reply_to, Some(message));
    assert_eq!(reply_quote.as_deref(), Some("shipping on friday"));
    assert!(!silent);
    assert!(app.reply_quote.is_none() && app.reply_to.is_none());

    press(&mut app, KeyCode::Esc);
    press(&mut app, KeyCode::Char('Q'));
    type_and_enter(&mut app, "friday");
    press(&mut app, KeyCode::Esc);
    assert!(app.reply_quote.is_none() && app.reply_to.is_none());
}

#[test]
fn silent_and_scheduled_sends_from_the_composer() {
    let (mut app, room, _, _) = chat_app();
    app.focus = Focus::Input;
    let actions = type_and_enter(&mut app, "/silent good night");
    assert!(matches!(actions.as_slice(),
        [Action::Chat(ChatCommand::Send { content, silent: true, .. })] if content == "good night"));
    let actions = type_and_enter(&mut app, "/schedule 1h ping");
    assert!(matches!(actions.as_slice(),
        [Action::Messaging(MessagingAction::Schedule { room_id, content, silent: false, .. })]
            if *room_id == room && content == "ping"));
    // A misused command keeps the draft and explains itself.
    assert!(type_and_enter(&mut app, "/schedule later ping").is_empty());
    assert_eq!(app.compose.value(), "/schedule later ping");
    assert_eq!(app.status, SCHEDULE_USAGE);
}

fn scheduled(id: u128, content: &str) -> ScheduledMessage {
    ScheduledMessage {
        id: Uuid::from_u128(id),
        content: content.into(),
        scheduled_at: "2026-10-02T08:00:00Z".into(),
        silent: id == 2,
    }
}

#[test]
fn scheduled_dialog_sends_now_cancels_and_reloads() {
    let (mut app, room, _, _) = chat_app();
    assert!(
        matches!(app.handle_key(KeyEvent::new(KeyCode::Char('S'), KeyModifiers::SHIFT)).as_slice(),
        [Action::Messaging(MessagingAction::LoadScheduled(r))] if *r == room)
    );
    app.apply_event(AppEvent::Messaging(MessagingEvent::Scheduled {
        room_id: room,
        result: Ok(vec![scheduled(1, "first"), scheduled(2, "second")]),
    }));
    let screen = draw(&mut app);
    assert!(
        screen.contains("Scheduled messages") && screen.contains("second"),
        "{screen}"
    );
    assert!(screen.contains("silent"), "{screen}");
    press(&mut app, KeyCode::Down);
    assert!(matches!(press(&mut app, KeyCode::Enter).as_slice(),
        [Action::Messaging(MessagingAction::SendScheduledNow { id, .. })] if *id == Uuid::from_u128(2)));
    assert!(matches!(press(&mut app, KeyCode::Char('x')).as_slice(),
        [Action::Messaging(MessagingAction::CancelScheduled { id, .. })] if *id == Uuid::from_u128(2)));
    let reload = app.apply_event(AppEvent::Messaging(MessagingEvent::ScheduledChanged {
        room_id: room,
        result: Ok("Scheduled message cancelled".into()),
    }));
    assert!(matches!(
        reload.as_slice(),
        [Action::Messaging(MessagingAction::LoadScheduled(_))]
    ));
    assert_eq!(app.status, "Scheduled message cancelled");
    press(&mut app, KeyCode::Esc);
    assert!(app.dialog.is_none());
}

#[test]
fn chat_search_jumps_to_a_loaded_message_or_reconnects_to_it() {
    let (mut app, room, message, _) = chat_app();
    press(&mut app, KeyCode::Char('/'));
    let actions = type_and_enter(&mut app, "friday");
    assert!(matches!(actions.as_slice(),
        [Action::Messaging(MessagingAction::Search { room_id, query, .. })] if *room_id == room && query == "friday"));
    let found = |id| FoundMessage {
        id,
        sender: "bob".into(),
        content: "are we shipping on friday".into(),
        created_at: "2026-10-01T09:00:00Z".into(),
    };
    let older = Uuid::from_u128(99);
    app.apply_event(AppEvent::Messaging(MessagingEvent::Found {
        query: "friday".into(),
        result: Ok(vec![found(message), found(older)]),
    }));
    assert!(draw(&mut app).contains("Search · friday"));
    app.message_index = 5;
    assert!(press(&mut app, KeyCode::Enter).is_empty());
    assert_eq!(app.message_index, 0);
    app.dialog = Some(Dialog::ChatSearch {
        query: "friday".into(),
        items: vec![found(older)],
        selected: 0,
    });
    assert!(matches!(press(&mut app, KeyCode::Enter).as_slice(),
        [Action::ConnectChat { target_message: Some(t), .. }] if *t == older));
}

#[test]
fn invite_links_join_connect_or_wait_for_approval() {
    let (mut app, _, _, _) = chat_app();
    app.focus = Focus::List;
    app.handle_key(KeyEvent::new(KeyCode::Char('J'), KeyModifiers::SHIFT));
    assert!(type_and_enter(&mut app, "not a link").is_empty());
    assert_eq!(app.status, "That is not an invite link");
    app.handle_key(KeyEvent::new(KeyCode::Char('J'), KeyModifiers::SHIFT));
    let actions = type_and_enter(&mut app, "https://chat.example/joinchat/AbCdEfGhIjKlMnOp");
    assert!(matches!(actions.as_slice(),
        [Action::Messaging(MessagingAction::JoinInvite(t))] if t == "AbCdEfGhIjKlMnOp"));
    let chat = Uuid::from_u128(77);
    let actions = app.apply_event(AppEvent::Messaging(MessagingEvent::Joined(Ok(
        InviteJoin {
            status: "active".into(),
            chat_id: Some(chat),
        },
    ))));
    assert!(matches!(actions.as_slice(),
        [Action::LoadConversations, Action::ConnectChat { room_id, .. }] if *room_id == chat));
    app.apply_event(AppEvent::Messaging(MessagingEvent::Joined(Ok(
        InviteJoin {
            status: "pending".into(),
            chat_id: None,
        },
    ))));
    assert_eq!(app.status, "Join request sent; an admin must approve it");
}

#[test]
fn reactions_are_counted_removable_and_drawn_with_reply_context() {
    let (mut app, room, message, _) = chat_app();
    assert!(matches!(app.reaction_action(message, "-👍").as_slice(),
        [Action::Chat(ChatCommand::React { active: false, emoji, .. })] if emoji == "👍"));
    assert!(app.reaction_action(message, " - ").is_empty());
    let bob = Uuid::from_u128(5);
    app.apply_chat_event(
        room,
        ChatEvent::ReactionChanged {
            message_id: message,
            emoji: "👍".into(),
            user_id: Some(bob),
            active: true,
        },
    );
    let reply: crate::client_chat_extras::MessageExtras = serde_json::from_value(serde_json::json!({
        "silent": true,
        "reply_to": { "message_id": message, "sender": "bob", "content": "are we shipping on friday",
            "recalled": false, "quote": { "text": "friday", "offset": 19 } } })).unwrap();
    let mut answer = app.messages[0].clone();
    answer.id = Uuid::from_u128(22);
    answer.content = "yes".into();
    answer.extras = Box::new(reply);
    app.messages.push(answer);
    app.typing_user = Some("bob is recording a voice message".into());
    let screen = draw(&mut app);
    assert!(screen.contains("👍 1"), "{screen}");
    assert!(screen.contains("↳ bob: “friday”"), "{screen}");
    assert!(screen.contains("· silent"), "{screen}");
    assert!(
        screen.contains("bob is recording a voice message"),
        "{screen}"
    );
}

fn draw(app: &mut App) -> String {
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(110, 30)).unwrap();
    terminal
        .draw(|frame| crate::client_tui::render::render(frame, app))
        .unwrap();
    terminal.backend().to_string()
}
