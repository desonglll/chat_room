//! Shared list selection and compact timestamp presentation for TUI views.

use ratatui::{
    layout::{Margin, Rect},
    style::{Color, Modifier, Style},
    widgets::{
        Block, List, ListItem, ListState, Paragraph, Scrollbar, ScrollbarOrientation,
        ScrollbarState,
    },
    Frame,
};

use super::render::clean;

pub(super) fn render_list(
    frame: &mut Frame<'_>,
    area: Rect,
    items: Vec<ListItem<'_>>,
    selected: usize,
    title: &str,
    focused: bool,
    empty_message: &str,
) {
    let selected = selected.min(items.len().saturating_sub(1));
    let panel_title = if items.is_empty() {
        format!(" {title} ")
    } else {
        format!(" {title}  {}/{} ", selected + 1, items.len())
    };
    let border = if focused {
        Color::Cyan
    } else {
        Color::DarkGray
    };
    let block = Block::bordered()
        .title(panel_title)
        .border_style(Style::default().fg(border));
    if items.is_empty() {
        frame.render_widget(
            Paragraph::new(format!("  {empty_message}"))
                .style(Style::default().fg(Color::DarkGray))
                .block(block),
            area,
        );
        return;
    }

    let content_height = items.iter().map(ListItem::height).sum::<usize>();
    let scroll_position = items
        .iter()
        .take(selected)
        .map(ListItem::height)
        .sum::<usize>();
    let viewport_height = area.height.saturating_sub(2) as usize;
    let mut state = ListState::default().with_selected(Some(selected));
    let highlight = if focused {
        Style::default()
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };
    let list = List::new(items)
        .block(block)
        .highlight_symbol(if focused { "> " } else { "  " })
        .highlight_style(highlight);
    frame.render_stateful_widget(list, area, &mut state);
    if content_height > viewport_height {
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(None)
            .end_symbol(None)
            .track_symbol(Some("│"))
            .thumb_symbol("┃")
            .track_style(Style::default().fg(Color::DarkGray))
            .thumb_style(Style::default().fg(border));
        let mut scrollbar_state = ScrollbarState::new(content_height)
            .position(scroll_position)
            .viewport_content_length(viewport_height);
        frame.render_stateful_widget(
            scrollbar,
            area.inner(Margin {
                vertical: 1,
                horizontal: 0,
            }),
            &mut scrollbar_state,
        );
    }
}

pub(super) fn one_line(value: &str) -> String {
    clean(value).lines().next().unwrap_or_default().to_string()
}

pub(super) fn short_time(value: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(value)
        .map(|time| {
            time.with_timezone(&chrono::Local)
                .format("%m-%d %H:%M")
                .to_string()
        })
        .unwrap_or_else(|_| one_line(value))
}

#[cfg(test)]
mod tests {
    use ratatui::{backend::TestBackend, Terminal};

    use super::*;

    #[test]
    fn unfocused_list_does_not_render_a_selection_highlight() {
        let backend = TestBackend::new(20, 4);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                render_list(
                    frame,
                    frame.area(),
                    vec![ListItem::new("Alpha")],
                    0,
                    "Items",
                    false,
                    "No items",
                );
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let alpha = (0..buffer.area.height)
            .flat_map(|y| (0..buffer.area.width).map(move |x| (x, y)))
            .find_map(|position| buffer.cell(position).filter(|cell| cell.symbol() == "A"))
            .expect("rendered list item");
        assert_eq!(alpha.bg, Color::Reset);
        assert!(terminal.backend().to_string().contains("Items  1/1"));
    }

    #[test]
    fn long_list_keeps_the_selection_visible_and_shows_scroll_position() {
        let backend = TestBackend::new(24, 6);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                let items = (0..10)
                    .map(|index| ListItem::new(format!("Item {index}")))
                    .collect();
                render_list(frame, frame.area(), items, 9, "Items", true, "No items");
            })
            .unwrap();

        let screen = terminal.backend().to_string();
        assert!(screen.contains("Items  10/10"));
        assert!(screen.contains("Item 9"));
        assert!(screen.contains('┃'));
    }
}
