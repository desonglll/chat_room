//! TG-1205: the scheduled-messages and in-chat search dialogs.

use ratatui::{
    layout::{Constraint, Layout},
    style::{Color, Style},
    text::Line,
    widgets::{Block, Clear, List, ListItem, ListState, Paragraph},
    Frame,
};

use super::messaging::local_time;
use super::model::{App, Dialog};
use super::render::centered;

pub fn render(frame: &mut Frame<'_>, dialog: &Dialog) {
    let (title, rows, selected, hint, empty) = match dialog {
        Dialog::Scheduled {
            items, selected, ..
        } => (
            "Scheduled messages".to_string(),
            items
                .iter()
                .map(|item| {
                    let silent = if item.silent { " · silent" } else { "" };
                    format!(
                        "{}{silent}  {}",
                        local_time(&item.scheduled_at),
                        short(&item.content, 52)
                    )
                })
                .collect::<Vec<_>>(),
            *selected,
            "Enter send now  x cancel it  Esc close",
            "Nothing scheduled — /schedule 2h <message> in the composer",
        ),
        Dialog::ChatSearch {
            query,
            items,
            selected,
        } => (
            format!("Search · {}", short(query, 40)),
            items
                .iter()
                .map(|item| {
                    format!(
                        "{}  {}: {}",
                        local_time(&item.created_at),
                        short(&item.sender, 16),
                        short(&item.content, 44)
                    )
                })
                .collect::<Vec<_>>(),
            *selected,
            "Enter jump to message  Esc close",
            "No messages match",
        ),
        _ => return,
    };
    let area = centered(frame.area(), 76, 18);
    frame.render_widget(Clear, area);
    let parts = Layout::vertical([Constraint::Min(3), Constraint::Length(1)]).split(area);
    let block = Block::bordered()
        .title(title)
        .border_style(Style::default().fg(Color::Cyan));
    if rows.is_empty() {
        frame.render_widget(
            Paragraph::new(empty)
                .style(Style::default().fg(Color::DarkGray))
                .block(block),
            parts[0],
        );
    } else {
        let mut state = ListState::default().with_selected(Some(selected));
        frame.render_stateful_widget(
            List::new(rows.into_iter().map(|row| ListItem::new(Line::raw(row))))
                .block(block)
                .highlight_style(Style::default().fg(Color::Yellow).bold())
                .highlight_symbol("› "),
            parts[0],
            &mut state,
        );
    }
    frame.render_widget(
        Paragraph::new(hint).style(Style::default().fg(Color::DarkGray)),
        parts[1],
    );
}

/// The composer's title: the slash commands, or what the message will reply to / quote.
pub fn composer_title(app: &App) -> String {
    match (app.reply_to, &app.reply_quote) {
        (None, _) => "Message  (/silent /schedule /scheduled /search /join)".into(),
        (Some(id), None) => format!("Reply to #{}", &id.to_string()[..8]),
        (Some(id), Some(quote)) => format!(
            "Reply to #{} quoting “{}”",
            &id.to_string()[..8],
            short(quote, 32)
        ),
    }
}

/// One line of at most `limit` characters, control characters flattened.
pub fn short(value: &str, limit: usize) -> String {
    let flat: String = value
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    if flat.chars().count() <= limit {
        return flat;
    }
    let cut: String = flat.chars().take(limit.saturating_sub(1)).collect();
    format!("{cut}…")
}
