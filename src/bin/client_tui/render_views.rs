//! Content rendering for the five main TUI workspaces.

use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, ListItem, Paragraph, Wrap},
    Frame,
};

use super::{
    model::{App, Focus},
    render::clean,
    render_list::{render_list, short_time},
};

const ACCENT: Color = Color::Cyan;
const MUTED: Color = Color::DarkGray;

pub(super) fn search(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let items = app
        .search_results
        .iter()
        .map(|result| {
            let file = result
                .attachment_file_name
                .as_deref()
                .map(|name| format!("  [file: {}]", clean(name)))
                .unwrap_or_default();
            ListItem::new(vec![
                Line::from(vec![
                    Span::styled(
                        clean(&result.conversation_title),
                        Style::default().fg(ACCENT),
                    ),
                    Span::raw(format!(
                        "  {}  {}",
                        clean(&result.sender),
                        short_time(&result.created_at)
                    )),
                    Span::styled(
                        format!("  [{}]{}", clean(&result.content_type), file),
                        Style::default().fg(Color::Green),
                    ),
                ]),
                Line::raw(clean(&result.excerpt)),
            ])
        })
        .collect();
    render_list(
        frame,
        area,
        items,
        app.search_index,
        "Global message search",
        app.focus == Focus::List,
        "No search results",
    );
}

pub(super) fn notifications(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let items = app
        .notifications
        .iter()
        .map(|notification| {
            let marker = if notification.read_at.is_some() {
                " "
            } else {
                "*"
            };
            ListItem::new(vec![
                Line::from(vec![
                    Span::styled(
                        format!("{marker} {}", clean(&notification.kind)),
                        Style::default().fg(ACCENT),
                    ),
                    Span::styled(
                        format!("  {}", short_time(&notification.created_at)),
                        Style::default().fg(MUTED),
                    ),
                    Span::styled(
                        notification
                            .room_name
                            .as_deref()
                            .map(|name| format!("  {}", clean(name)))
                            .unwrap_or_default(),
                        Style::default().fg(Color::Green),
                    ),
                ]),
                Line::raw(clean(&notification.summary)),
            ])
        })
        .collect();
    render_list(
        frame,
        area,
        items,
        app.notification_index,
        "Notifications",
        true,
        "You're all caught up",
    );
}

pub(super) fn favorites(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let columns = Layout::horizontal([Constraint::Percentage(38), Constraint::Percentage(62)])
        .spacing(1)
        .split(area);
    let items = app
        .favorites
        .iter()
        .map(|favorite| {
            ListItem::new(vec![
                Line::styled(clean(&favorite.title), Style::default().fg(ACCENT)),
                Line::styled(
                    format!("{}  {}", favorite.kind, short_time(&favorite.updated_at)),
                    Style::default().fg(MUTED),
                ),
            ])
        })
        .collect();
    render_list(
        frame,
        columns[0],
        items,
        app.favorite_index,
        "Favorites",
        true,
        "No favorites saved",
    );
    let detail = app.selected_favorite().map_or_else(
        || "No favorite selected".into(),
        |favorite| {
            format!(
                "{}\n\n{}\n\nOwner access: {}\nSource: {} / {}",
                clean(&favorite.title),
                clean(&favorite.content),
                clean(&favorite.access),
                clean(&favorite.source_room_name),
                clean(&favorite.source_sender)
            )
        },
    );
    frame.render_widget(
        Paragraph::new(detail)
            .wrap(Wrap { trim: false })
            .block(Block::bordered().title("Detail")),
        columns[1],
    );
}

pub(super) fn ai(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let columns = Layout::horizontal([Constraint::Length(29), Constraint::Min(20)])
        .spacing(1)
        .split(area);
    let threads = app
        .ai_threads
        .iter()
        .map(|thread| {
            ListItem::new(vec![
                Line::styled(clean(&thread.title), Style::default().fg(ACCENT)),
                Line::styled(
                    format!(
                        "{}{}",
                        short_time(&thread.updated_at),
                        thread.room_id.map(|_| "  room").unwrap_or_default()
                    ),
                    Style::default().fg(MUTED),
                ),
            ])
        })
        .collect();
    render_list(
        frame,
        columns[0],
        threads,
        app.ai_thread_index,
        "AI threads",
        app.focus == Focus::List,
        "No AI threads yet",
    );
    let messages = app
        .ai_messages
        .iter()
        .map(|message| {
            let color = if message.role == "user" {
                Color::Green
            } else {
                ACCENT
            };
            ListItem::new(vec![
                Line::styled(
                    format!(
                        "{}  #{}",
                        clean(&message.role),
                        &message.id.to_string()[..8]
                    ),
                    Style::default().fg(color).bold(),
                ),
                Line::raw(clean(&message.content)),
                Line::styled(clean(&message.status), Style::default().fg(MUTED)),
            ])
        })
        .collect();
    render_list(
        frame,
        columns[1],
        messages,
        app.ai_message_index,
        "AI conversation",
        false,
        "Select or create a thread",
    );
}
