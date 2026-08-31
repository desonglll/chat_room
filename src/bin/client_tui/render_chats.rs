//! Responsive conversation and message panels for the chat workspace.

use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span, Text},
    widgets::ListItem,
    Frame,
};

use super::{
    model::{App, Focus},
    render::clean,
    render_list::{one_line, render_list, short_time},
};

const ACCENT: Color = Color::Cyan;
const MUTED: Color = Color::DarkGray;
const SINGLE_PANEL_WIDTH: u16 = 96;

pub(super) fn render(frame: &mut Frame<'_>, app: &App, area: Rect) {
    if area.width < SINGLE_PANEL_WIDTH {
        if app.focus == Focus::List {
            render_conversations(frame, app, area);
        } else {
            render_messages(frame, app, area);
        }
        return;
    }

    let sidebar_width = (area.width / 3).clamp(30, 42);
    let columns = Layout::horizontal([Constraint::Length(sidebar_width), Constraint::Min(30)])
        .spacing(1)
        .split(area);
    render_conversations(frame, app, columns[0]);
    render_messages(frame, app, columns[1]);
}

fn render_conversations(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let conversations = app
        .conversations
        .iter()
        .map(|conversation| {
            let active = app.active_room == Some(conversation.room_id);
            let marker = if conversation.preferences.muted_until.is_some() {
                "!"
            } else if conversation.preferences.is_pinned {
                "*"
            } else if conversation.preferences.is_archived {
                "~"
            } else {
                " "
            };
            let active_marker = if active { "●" } else { " " };
            let unread = if conversation.unread_count > 0 {
                format!(" ({})", conversation.unread_count)
            } else {
                String::new()
            };
            let preview = conversation
                .last_message
                .as_ref()
                .map(|message| {
                    if message.recalled {
                        "message recalled".into()
                    } else {
                        format!("{}: {}", clean(&message.sender), one_line(&message.content))
                    }
                })
                .unwrap_or_else(|| "No messages".into());
            let kind = if conversation.kind == "direct" {
                "@"
            } else {
                "#"
            };
            ListItem::new(vec![
                Line::styled(
                    format!(
                        "{active_marker}{marker}{kind} {}{unread}",
                        clean(&conversation.title)
                    ),
                    if active {
                        Style::default().fg(ACCENT).bold()
                    } else {
                        Style::default()
                    },
                ),
                Line::styled(preview, Style::default().fg(MUTED)),
            ])
        })
        .collect::<Vec<_>>();
    render_list(
        frame,
        area,
        conversations,
        app.conversation_index,
        "Conversations",
        app.focus == Focus::List,
        "No conversations yet",
    );
}

fn render_messages(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let content_width = area.width.saturating_sub(6).max(1) as usize;
    let messages = app
        .messages
        .iter()
        .map(|message| {
            let edited = if message.edited { " · edited" } else { "" };
            let id = message.id.to_string();
            let author_color = if message.sender == app.username {
                Color::Green
            } else {
                ACCENT
            };
            let heading = Line::from(vec![
                Span::styled(short_time(&message.timestamp), Style::default().fg(MUTED)),
                Span::styled(
                    format!("  {}", clean(&message.sender)),
                    Style::default().fg(author_color).bold(),
                ),
                Span::styled(
                    format!("  #{}{edited}", &id[..8]),
                    Style::default().fg(MUTED),
                ),
            ]);
            let mut lines = vec![heading];
            if message.recalled {
                lines.push(Line::styled("message recalled", Style::default().fg(MUTED)));
            } else {
                lines.extend(wrap_message(&message.content, content_width));
            }
            if let Some(file) = &message.attachment {
                let kind = file.mime_type.split('/').next().unwrap_or("file");
                lines.push(Line::styled(
                    format!(
                        "[{kind}] {}  {}  #{}",
                        clean(&file.file_name),
                        crate::client_media::format_size(file.size_bytes),
                        &file.id.to_string()[..8]
                    ),
                    Style::default().fg(Color::Green),
                ));
            }
            ListItem::new(Text::from(lines))
        })
        .collect::<Vec<_>>();
    let title = if app.active_room_name.is_empty() {
        "Messages".into()
    } else {
        format!("Messages · {}", clean(&app.active_room_name))
    };
    let empty = if app.active_room.is_some() {
        "No messages yet"
    } else {
        "Select a conversation"
    };
    render_list(
        frame,
        area,
        messages,
        app.message_index,
        &title,
        app.focus == Focus::Content,
        empty,
    );
}

fn wrap_message(value: &str, width: usize) -> Vec<Line<'static>> {
    let mut output = Vec::new();
    for source_line in clean(value).split('\n') {
        if source_line.is_empty() {
            output.push(Line::raw(""));
            continue;
        }
        let mut current = String::new();
        for character in source_line.chars() {
            current.push(character);
            if Line::from(current.as_str()).width() > width && current.chars().count() > 1 {
                let overflow = current.pop().expect("line has at least two characters");
                output.push(Line::raw(std::mem::take(&mut current)));
                current.push(overflow);
            }
        }
        if !current.is_empty() {
            output.push(Line::raw(current));
        }
    }
    if output.is_empty() {
        output.push(Line::raw(""));
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wraps_cjk_and_preserves_explicit_lines() {
        let lines = wrap_message("你好世界\nnext", 4);
        let rendered = lines.iter().map(ToString::to_string).collect::<Vec<_>>();
        assert_eq!(rendered, ["你好", "世界", "next"]);
    }
}
