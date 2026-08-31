//! Full-screen Ratatui client runtime.

use std::io::IsTerminal;

use anyhow::{bail, Context, Result};
use crossterm::event::{Event, EventStream, KeyEventKind};
use futures_util::StreamExt;
use tokio::{sync::mpsc, time::Duration};
use uuid::Uuid;

use crate::client_auth;

mod dispatch;
mod input;
mod model;
mod navigation;
mod render;
mod render_chats;
mod render_dialog;
mod render_list;
mod render_views;
mod update;
mod update_dialog;
mod update_events;
mod update_session;
mod update_views;

use model::{Action, App, AppEvent};

pub async fn run(server: &str, initial_room: Option<(Uuid, Option<String>)>) -> Result<()> {
    if !std::io::stdout().is_terminal() || !std::io::stdin().is_terminal() {
        bail!("the terminal UI requires an interactive terminal");
    }
    let config = client_auth::load_config()?;
    let mut app = App::new(server.to_string(), config, initial_room);
    let (event_tx, mut event_rx) = mpsc::unbounded_channel::<AppEvent>();
    let mut terminal = ratatui::try_init().context("initialize terminal UI")?;
    let result = run_loop(&mut terminal, &mut app, event_tx, &mut event_rx).await;
    ratatui::restore();
    result
}

async fn run_loop(
    terminal: &mut ratatui::DefaultTerminal,
    app: &mut App,
    event_tx: mpsc::UnboundedSender<AppEvent>,
    event_rx: &mut mpsc::UnboundedReceiver<AppEvent>,
) -> Result<()> {
    let mut terminal_events = EventStream::new();
    let mut redraw = tokio::time::interval(Duration::from_millis(100));
    redraw.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    for action in app.startup_actions() {
        dispatch::action(app, action, event_tx.clone());
    }
    loop {
        terminal.draw(|frame| render::render(frame, app))?;
        tokio::select! {
            maybe_event = terminal_events.next() => match maybe_event {
                Some(Ok(Event::Key(key))) if matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) => {
                    let actions = app.handle_key(key);
                    if dispatch_actions(app, actions, event_tx.clone()) {
                        break;
                    }
                }
                Some(Ok(_)) => {}
                Some(Err(error)) => return Err(error).context("read terminal event"),
                None => break,
            },
            Some(event) = event_rx.recv() => {
                let actions = app.apply_event(event);
                if dispatch_actions(app, actions, event_tx.clone()) {
                    break;
                }
            }
            _ = redraw.tick() => {}
        }
    }
    if let Some(chat) = &app.chat {
        let _ = chat.send(crate::client_chat::ChatCommand::Close);
    }
    Ok(())
}

fn dispatch_actions(
    app: &mut App,
    actions: Vec<Action>,
    event_tx: mpsc::UnboundedSender<AppEvent>,
) -> bool {
    for action in actions {
        if matches!(action, Action::Quit) {
            return true;
        }
        dispatch::action(app, action, event_tx.clone());
    }
    false
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::{backend::TestBackend, Terminal};

    use super::*;
    use crate::{
        client_api::{Conversation, ConversationPreferences},
        client_chat::{ChatCommand, ChatEvent, ChatMessage},
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

    fn render_chat(app: &mut App, width: u16) -> String {
        let backend = TestBackend::new(width, 18);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| render_chats::render(frame, app, frame.area()))
            .unwrap();
        terminal.backend().to_string()
    }

    #[test]
    fn sign_in_screen_renders_without_leaking_password() {
        let mut app = App::new(
            "http://127.0.0.1:3000".into(),
            client_auth::UserConfig::default(),
            None,
        );
        app.auth_password.set("private-password");
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| render::render(frame, &mut app))
            .unwrap();
        let screen = terminal.backend().to_string();
        assert!(screen.contains("Echo Gate"));
        assert!(!screen.contains("private-password"));
        assert!(screen.contains("****************"));
    }

    #[test]
    fn startup_with_saved_session_validates_before_loading_data() {
        let app = App::new(
            "http://localhost".into(),
            client_auth::UserConfig {
                username: "alice".into(),
                token: Some(Uuid::nil()),
            },
            None,
        );
        assert!(matches!(
            app.startup_actions().as_slice(),
            [Action::ValidateSession]
        ));
    }

    #[test]
    fn help_overlay_shows_global_and_contextual_shortcuts() {
        let mut app = App::new(
            "http://localhost".into(),
            client_auth::UserConfig::default(),
            None,
        );
        app.screen = model::Screen::Main;
        app.handle_key(KeyEvent::new(KeyCode::F(1), KeyModifiers::NONE));
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| render::render(frame, &mut app))
            .unwrap();
        let screen = terminal.backend().to_string();
        assert!(screen.contains("Keyboard shortcuts"));
        assert!(screen.contains("Change view"));
        assert!(screen.contains("Select room"));

        let backend = TestBackend::new(60, 16);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| render::render(frame, &mut app))
            .unwrap();
        assert!(terminal
            .backend()
            .to_string()
            .contains("Keyboard shortcuts"));
    }

    #[test]
    fn chat_layout_uses_one_focused_panel_when_narrow() {
        let room_id = Uuid::new_v4();
        let mut app = App::new(
            "http://localhost".into(),
            client_auth::UserConfig::default(),
            None,
        );
        app.active_room = Some(room_id);
        app.active_room_name = "Test room".into();
        app.conversations = vec![conversation(room_id, "Test room")];

        app.focus = model::Focus::List;
        let rooms = render_chat(&mut app, 80);
        assert!(rooms.contains("Conversations"));
        assert!(!rooms.contains("Messages · Test room"));

        app.focus = model::Focus::Content;
        let messages = render_chat(&mut app, 80);
        assert!(!messages.contains("Conversations"));
        assert!(messages.contains("Messages · Test room"));

        let wide = render_chat(&mut app, 120);
        assert!(wide.contains("Conversations"));
        assert!(wide.contains("Messages · Test room"));
        assert!(wide.contains("● # Test room"));
    }

    #[test]
    fn newly_sent_message_stays_visible_while_composer_has_focus() {
        let room_id = Uuid::new_v4();
        let mut app = App::new(
            "http://localhost".into(),
            client_auth::UserConfig::default(),
            None,
        );
        app.screen = model::Screen::Main;
        app.active_room = Some(room_id);
        app.active_room_name = "Test room".into();
        app.focus = model::Focus::Input;

        for index in 0..8 {
            app.apply_chat_event(
                room_id,
                ChatEvent::Message(ChatMessage {
                    id: Uuid::new_v4(),
                    sender: "alice".into(),
                    content: format!("older message {index}"),
                    attachment: None,
                    timestamp: "2026-08-31T12:00:00Z".into(),
                    recalled: false,
                    edited: false,
                }),
            );
        }
        app.compose.set("newly sent message");
        let actions = app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert!(matches!(
            actions.as_slice(),
            [Action::Chat(ChatCommand::Send { content, .. })] if content == "newly sent message"
        ));
        app.apply_chat_event(
            room_id,
            ChatEvent::Message(ChatMessage {
                id: Uuid::new_v4(),
                sender: "alice".into(),
                content: "newly sent message".into(),
                attachment: None,
                timestamp: "2026-08-31T12:00:00Z".into(),
                recalled: false,
                edited: false,
            }),
        );
        assert_eq!(app.message_index, 8);

        app.focus = model::Focus::Content;
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| render::render(frame, &mut app))
            .unwrap();
        assert!(terminal
            .backend()
            .to_string()
            .contains("newly sent message"));

        app.focus = model::Focus::Input;
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| render::render(frame, &mut app))
            .unwrap();
        assert!(terminal
            .backend()
            .to_string()
            .contains("newly sent message"));
    }
}
