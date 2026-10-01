//! TG-907 drawing: the Contacts tab and the open chat's pinned-message title suffix.

use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::ListItem,
    Frame,
};

use super::{
    model::{App, Focus},
    render::clean,
    render_list::render_list,
    social::ContactRow,
};

const ACCENT: Color = Color::Cyan;
const MUTED: Color = Color::DarkGray;

pub(super) fn contacts(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let items = app
        .social
        .contacts
        .rows()
        .into_iter()
        .map(|row| match row {
            ContactRow::Request { label, .. } => ListItem::new(vec![
                Line::from(vec![
                    Span::styled("Request  ", Style::default().fg(Color::Yellow).bold()),
                    Span::raw(clean(&label)),
                ]),
                Line::styled("a accept  x decline", Style::default().fg(MUTED)),
            ]),
            ContactRow::Friend {
                name,
                status,
                online,
                ..
            } => ListItem::new(vec![
                Line::from(vec![
                    Span::styled(
                        if online { "● " } else { "  " },
                        Style::default().fg(Color::Green),
                    ),
                    Span::styled(clean(&name), Style::default().bold()),
                ]),
                Line::styled(
                    format!("  {status}"),
                    Style::default().fg(if online { ACCENT } else { MUTED }),
                ),
            ]),
        })
        .collect();
    render_list(
        frame,
        area,
        items,
        app.social.index,
        "Contacts · Enter chat · n add",
        app.focus == Focus::List,
        "No contacts yet — press n to add one",
    );
}

/// `· 📌 <newest pin> (+N)` for the messages pane title; empty without pins.
pub(super) fn pinned_suffix(app: &App) -> String {
    match app.social.pins.split_first() {
        None => String::new(),
        Some(((_, text), rest)) => {
            let text: String = clean(text).chars().take(40).collect();
            if rest.is_empty() {
                format!(" · 📌 {text}")
            } else {
                format!(" · 📌 {text} (+{})", rest.len())
            }
        }
    }
}

/// Whether the message is pinned (its row heading gets a marker).
pub(super) fn is_pinned(app: &App, message_id: uuid::Uuid) -> bool {
    app.social.pins.iter().any(|(id, _)| *id == message_id)
}
