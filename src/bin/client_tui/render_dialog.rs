//! Overlay rendering for TUI prompts, forms, and confirmations.

use ratatui::{
    layout::{Constraint, Layout},
    style::{Color, Style},
    text::{Line, Span, Text},
    widgets::{Block, Clear, List, ListItem, ListState, Paragraph},
    Frame,
};

use super::{
    model::{App, Dialog, Focus, View},
    render::{centered, render_input},
};

pub fn render(frame: &mut Frame<'_>, app: &App) {
    let Some(dialog) = app.dialog.as_ref() else {
        return;
    };
    match dialog {
        Dialog::Help => render_help(frame, app),
        Dialog::Scheduled { .. } | Dialog::ChatSearch { .. } => {
            super::render_messaging::render(frame, dialog)
        }
        Dialog::Prompt { title, input, .. } => {
            let area = centered(frame.area(), 54, 5);
            frame.render_widget(Clear, area);
            render_input(frame, area, title, input, true);
        }
        Dialog::CreateChat {
            name,
            password,
            field,
        } => {
            let area = centered(frame.area(), 58, 11);
            frame.render_widget(Clear, area);
            frame.render_widget(
                Block::bordered()
                    .title("Create chat")
                    .border_style(Style::default().fg(Color::Cyan)),
                area,
            );
            let inner = area.inner(ratatui::layout::Margin {
                horizontal: 2,
                vertical: 1,
            });
            let rows = Layout::vertical([
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Length(1),
            ])
            .split(inner);
            render_input(frame, rows[0], "Chat name", name, *field == 0);
            render_input(frame, rows[1], "Password (optional)", password, *field == 1);
            frame.render_widget(
                Paragraph::new("Enter advances/submits  Esc cancels")
                    .style(Style::default().fg(Color::DarkGray)),
                rows[2],
            );
        }
        Dialog::Chats { items, selected } => {
            let area = centered(frame.area(), 66, 18);
            frame.render_widget(Clear, area);
            let rows = Layout::vertical([Constraint::Min(4), Constraint::Length(1)]).split(area);
            let list_items = items
                .iter()
                .map(|chat| {
                    let access = if chat.has_password {
                        "private"
                    } else {
                        "public"
                    };
                    let membership = chat.membership_status.as_deref().unwrap_or("not joined");
                    ListItem::new(Line::from(format!(
                        "{}  [{}]  {}",
                        chat.title, access, membership
                    )))
                })
                .collect::<Vec<_>>();
            let mut state =
                ListState::default().with_selected((!list_items.is_empty()).then_some(*selected));
            frame.render_stateful_widget(
                List::new(list_items)
                    .block(Block::bordered().title("Discover chats"))
                    .highlight_symbol("> ")
                    .highlight_style(Style::default().bg(Color::DarkGray)),
                rows[0],
                &mut state,
            );
            frame.render_widget(
                Paragraph::new("Enter joins  Esc closes")
                    .style(Style::default().fg(Color::DarkGray)),
                rows[1],
            );
        }
        Dialog::FavoriteEditor {
            id,
            title,
            content,
            field,
            ..
        } => {
            let area = centered(frame.area(), 66, 11);
            frame.render_widget(Clear, area);
            frame.render_widget(
                Block::bordered()
                    .title(if id.is_some() {
                        "Edit favorite"
                    } else {
                        "New favorite"
                    })
                    .border_style(Style::default().fg(Color::Cyan)),
                area,
            );
            let inner = area.inner(ratatui::layout::Margin {
                horizontal: 2,
                vertical: 1,
            });
            let rows = Layout::vertical([
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Length(1),
            ])
            .split(inner);
            render_input(frame, rows[0], "Title", title, *field == 0);
            render_input(frame, rows[1], "Content", content, *field == 1);
            frame.render_widget(
                Paragraph::new("Enter advances/submits  Esc cancels")
                    .style(Style::default().fg(Color::DarkGray)),
                rows[2],
            );
        }
        Dialog::Confirm { title, .. } => {
            let area = centered(frame.area(), 48, 5);
            frame.render_widget(Clear, area);
            frame.render_widget(
                Paragraph::new("Enter/y confirms  n/Esc cancels")
                    .style(Style::default().fg(Color::Yellow))
                    .block(
                        Block::bordered()
                            .title(title.as_str())
                            .border_style(Style::default().fg(Color::Red)),
                    ),
                area,
            );
        }
    }
}

fn render_help(frame: &mut Frame<'_>, app: &App) {
    frame.render_widget(Clear, frame.area());
    let area = centered(frame.area(), 72, 21);
    frame.render_widget(
        Block::bordered()
            .title(format!(" Keyboard shortcuts · {} ", app.view.title()))
            .border_style(Style::default().fg(Color::Cyan)),
        area,
    );
    let inner = area.inner(ratatui::layout::Margin {
        horizontal: 2,
        vertical: 1,
    });
    let columns = Layout::horizontal([Constraint::Percentage(46), Constraint::Percentage(54)])
        .spacing(2)
        .split(inner);
    let global = [
        ("1–5", "Change view"),
        ("Tab", "Next focus"),
        ("Shift-Tab", "Previous focus"),
        ("C-s", "Search"),
        ("C-g", "Cancel"),
        ("r", "Refresh"),
        ("F9", "Log out"),
        ("q / Ctrl-C", "Quit"),
        ("F1 / ?", "Close help"),
    ];
    frame.render_widget(help_text("Global", &global), columns[0]);
    frame.render_widget(help_text("Current view", view_help(app)), columns[1]);
}

fn view_help(app: &App) -> &'static [(&'static str, &'static str)] {
    match (app.view, app.focus) {
        (View::Chats, Focus::List) => &[
            ("C-n / C-p", "Select chat"),
            ("C-v / M-v", "Move by page"),
            ("M-< / M->", "First / last"),
            ("Enter", "Open chat"),
            ("C-f / Right", "Messages"),
            ("n / g", "New / discover"),
            ("p a m t", "Chat preferences"),
            ("o", "Next chat folder"),
            ("J", "Join by invite link"),
            ("/", "Search open chat"),
        ],
        (View::Chats, Focus::Content) => &[
            ("C-n / C-p", "Select message"),
            ("C-b / Left", "Conversations"),
            ("i", "Write message"),
            ("R / e / x", "Reply / edit / recall"),
            ("+ / f", "React / favorite"),
            ("u / d", "Upload / download"),
            ("P / F", "Pin-unpin / forward"),
            ("V", "Vote in poll"),
            ("Q", "Quote part in reply"),
            ("/ / S", "Search chat / scheduled"),
        ],
        (View::Chats, Focus::Input) => &[
            ("Enter", "Send"),
            ("C-g / Esc", "Return to messages"),
            ("C-b / C-f", "Move cursor"),
            ("C-a / C-e", "Start / end"),
            ("C-d / C-k", "Delete / kill"),
            ("/silent …", "Send without sound"),
            ("/schedule 2h …", "Send later (S lists)"),
            ("/search /join", "Search chat / join link"),
        ],
        (View::Search, Focus::Input) => &[("Enter", "Search"), ("C-g / Esc", "Results")],
        (View::Search, _) => &[
            ("C-n / C-p", "Select result"),
            ("/ / i", "Search input"),
            ("Enter", "Open source"),
        ],
        (View::Notifications, _) => &[
            ("C-n / C-p", "Select notification"),
            ("Enter", "Open source"),
            ("a", "Mark all read"),
        ],
        (View::Favorites, _) => &[
            ("C-n / C-p", "Select favorite"),
            ("n / e / d", "New / edit / delete"),
            ("Enter", "Open source"),
        ],
        (View::Contacts, _) => &[
            ("C-n / C-p", "Select contact"),
            ("Enter", "Open private chat"),
            ("a / x", "Accept / decline request"),
            ("n", "Add by @username"),
        ],
        (View::Ai, Focus::Input) => &[("Enter", "Ask AI"), ("C-g / Esc", "Threads")],
        (View::Ai, _) => &[
            ("C-n / C-p", "Select thread"),
            ("n", "New thread"),
            ("Enter", "Open thread"),
            ("i", "Ask AI"),
        ],
    }
}

fn help_text(title: &'static str, bindings: &[(&str, &str)]) -> Paragraph<'static> {
    let mut lines = vec![
        Line::styled(title, Style::default().fg(Color::Cyan).bold()),
        Line::raw(""),
    ];
    lines.extend(bindings.iter().map(|(key, label)| {
        Line::from(vec![
            Span::styled(
                format!("{key:<13}"),
                Style::default().fg(Color::Yellow).bold(),
            ),
            Span::raw((*label).to_string()),
        ])
    }));
    Paragraph::new(Text::from(lines))
}
