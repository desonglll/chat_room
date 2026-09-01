//! Ratatui rendering for the terminal client's primary screens.

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Position, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Tabs},
    Frame,
};

use super::{
    input::TextField,
    model::{App, Focus, Screen, View},
};

const ACCENT: Color = Color::Cyan;
const ACTIVE: Color = Color::Yellow;
const MUTED: Color = Color::DarkGray;

pub fn render(frame: &mut Frame<'_>, app: &mut App) {
    let area = frame.area();
    if area.width < 60 || area.height < 16 {
        frame.render_widget(
            Paragraph::new("Echo Gate needs a terminal of at least 60 x 16")
                .alignment(Alignment::Center)
                .block(Block::bordered().title("Terminal too small")),
            area,
        );
        return;
    }
    match app.screen {
        Screen::SignIn => render_sign_in(frame, app, area),
        Screen::Main => render_main(frame, app, area),
    }
    if app.dialog.is_some() {
        super::render_dialog::render(frame, app);
    }
}

fn render_sign_in(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let box_area = centered(area, 54, 15);
    frame.render_widget(
        Block::bordered()
            .title(" Echo Gate ")
            .border_style(Style::default().fg(ACCENT)),
        box_area,
    );
    let inner = box_area.inner(ratatui::layout::Margin {
        horizontal: 2,
        vertical: 1,
    });
    let rows = Layout::vertical([
        Constraint::Length(2),
        Constraint::Length(3),
        Constraint::Length(3),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .split(inner);
    let title = match app.auth_mode {
        super::model::AuthMode::Login => "Sign in",
        super::model::AuthMode::Register => "Create account",
    };
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(title, Style::default().fg(ACTIVE).bold()),
            Span::styled("  F2 switches mode", Style::default().fg(MUTED)),
        ])),
        rows[0],
    );
    render_input(
        frame,
        rows[1],
        "Username",
        &app.auth_username,
        app.auth_field == 0,
    );
    render_input(
        frame,
        rows[2],
        "Password",
        &app.auth_password,
        app.auth_field == 1,
    );
    frame.render_widget(
        Paragraph::new(format!("Server  {}", app.server)).style(Style::default().fg(MUTED)),
        rows[3],
    );
    frame.render_widget(
        Paragraph::new(clean(&app.status)).style(Style::default().fg(ACTIVE)),
        rows[4],
    );
}

fn render_main(frame: &mut Frame<'_>, app: &mut App, area: Rect) {
    let composer_height = match app.view {
        View::Chats | View::Search | View::Ai => 3,
        View::Notifications | View::Favorites => 0,
    };
    let rows = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(2),
        Constraint::Min(5),
        Constraint::Length(composer_height),
        Constraint::Length(2),
    ])
    .split(area);
    render_header(frame, app, rows[0]);

    let tab_titles = View::ALL
        .iter()
        .enumerate()
        .map(|(index, view)| Line::from(format!(" {} {} ", index + 1, view.title())))
        .collect::<Vec<_>>();
    frame.render_widget(
        Tabs::new(tab_titles)
            .select(app.view.index())
            .block(Block::new().borders(Borders::BOTTOM))
            .highlight_style(Style::default().fg(ACTIVE).bold()),
        rows[1],
    );
    match app.view {
        View::Chats => super::render_chats::render(frame, app, rows[2]),
        View::Search => super::render_views::search(frame, app, rows[2]),
        View::Notifications => super::render_views::notifications(frame, app, rows[2]),
        View::Favorites => super::render_views::favorites(frame, app, rows[2]),
        View::Ai => super::render_views::ai(frame, app, rows[2]),
    }
    if composer_height > 0 {
        render_composer(frame, app, rows[3]);
    }
    render_footer(frame, app, rows[4]);
}

fn render_header(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let columns = Layout::horizontal([Constraint::Min(20), Constraint::Length(14)]).split(area);
    let header = Line::from(vec![
        Span::styled(
            " ECHO GATE ",
            Style::default().fg(Color::Black).bg(ACCENT).bold(),
        ),
        Span::raw("  "),
        Span::styled(clean(&app.username), Style::default().fg(Color::Green)),
        Span::styled(format!("  {}", app.server), Style::default().fg(MUTED)),
    ]);
    frame.render_widget(Paragraph::new(header), columns[0]);
    let (label, color) = if app.busy {
        ("SYNCING", ACTIVE)
    } else if app.chat.is_some() {
        ("CONNECTED", Color::Green)
    } else {
        ("READY", MUTED)
    };
    frame.render_widget(
        Paragraph::new(label)
            .alignment(Alignment::Right)
            .style(Style::default().fg(color).bold()),
        columns[1],
    );
}

fn render_composer(frame: &mut Frame<'_>, app: &App, area: Rect) {
    match app.view {
        View::Chats => {
            let title = app.reply_to.map_or_else(
                || "Message".into(),
                |id| format!("Reply to #{}", &id.to_string()[..8]),
            );
            render_input(frame, area, &title, &app.compose, app.focus == Focus::Input);
        }
        View::Search => render_input(
            frame,
            area,
            "Search query",
            &app.search_input,
            app.focus == Focus::Input,
        ),
        View::Ai => render_input(
            frame,
            area,
            "Ask AI",
            &app.ai_input,
            app.focus == Focus::Input,
        ),
        View::Notifications => frame.render_widget(
            Paragraph::new("Enter open  a mark all read  r refresh")
                .style(Style::default().fg(MUTED))
                .block(Block::bordered().title("Actions")),
            area,
        ),
        View::Favorites => frame.render_widget(
            Paragraph::new("n new  e edit  d delete  Enter open source")
                .style(Style::default().fg(MUTED))
                .block(Block::bordered().title("Actions")),
            area,
        ),
    }
}

fn render_footer(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let rows = Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).split(area);
    let status_columns =
        Layout::horizontal([Constraint::Min(20), Constraint::Length(14)]).split(rows[0]);
    let typing = app
        .typing_user
        .as_deref()
        .filter(|name| !name.is_empty())
        .map(|name| format!("  {} is typing", clean(name)))
        .unwrap_or_default();
    frame.render_widget(
        Paragraph::new(format!("{}{}", clean(&app.status), typing))
            .style(Style::default().fg(ACTIVE)),
        status_columns[0],
    );
    frame.render_widget(
        Paragraph::new(format!(" {} ", focus_label(app)))
            .alignment(Alignment::Right)
            .style(Style::default().fg(Color::Black).bg(ACCENT).bold()),
        status_columns[1],
    );
    let hint_columns =
        Layout::horizontal([Constraint::Min(24), Constraint::Length(24)]).split(rows[1]);
    frame.render_widget(
        Paragraph::new(shortcut_line(shortcuts(app))),
        hint_columns[0],
    );
    let global_hint = if app.focus == Focus::Input {
        "F1 Help  C-g Cancel"
    } else {
        "F1/? Help  q Quit"
    };
    frame.render_widget(
        Paragraph::new(global_hint)
            .alignment(Alignment::Right)
            .style(Style::default().fg(MUTED)),
        hint_columns[1],
    );
}

fn shortcuts(app: &App) -> &'static [(&'static str, &'static str)] {
    match (app.view, app.focus) {
        (View::Chats, Focus::List) => &[("C-n/p", "Move"), ("Enter", "Open"), ("C-f", "Messages")],
        (View::Chats, Focus::Content) => &[
            ("C-n/p", "Move"),
            ("i", "Write"),
            ("C-b", "Rooms"),
            ("R", "Reply"),
        ],
        (View::Chats, Focus::Input) => &[("Enter", "Send"), ("C-g", "Messages")],
        (View::Search, Focus::Input) => &[("Enter", "Search"), ("C-g", "Results")],
        (View::Search, _) => &[("C-n/p", "Move"), ("/", "Search"), ("Enter", "Open")],
        (View::Notifications, _) => &[("C-n/p", "Move"), ("Enter", "Open"), ("a", "Read all")],
        (View::Favorites, _) => &[("C-n/p", "Move"), ("n", "New"), ("e", "Edit")],
        (View::Ai, Focus::Input) => &[("Enter", "Ask"), ("C-g", "Threads")],
        (View::Ai, _) => &[("C-n/p", "Move"), ("n", "New"), ("i", "Ask")],
    }
}

fn shortcut_line(bindings: &[(&str, &str)]) -> Line<'static> {
    let mut spans = Vec::new();
    for (key, label) in bindings {
        spans.push(Span::styled(
            (*key).to_string(),
            Style::default().fg(ACTIVE).bold(),
        ));
        spans.push(Span::styled(
            format!(" {label}  "),
            Style::default().fg(MUTED),
        ));
    }
    Line::from(spans)
}

fn focus_label(app: &App) -> &'static str {
    match (app.view, app.focus) {
        (View::Chats, Focus::List) => "ROOMS",
        (View::Chats, Focus::Content) => "MESSAGES",
        (View::Chats, Focus::Input) => "COMPOSE",
        (View::Search, Focus::Input) => "SEARCH",
        (View::Ai, Focus::Input) => "ASK AI",
        (View::Search, _) => "RESULTS",
        (View::Notifications, _) => "NOTICES",
        (View::Favorites, _) => "FAVORITES",
        (View::Ai, _) => "THREADS",
    }
}

pub(super) fn render_input(
    frame: &mut Frame<'_>,
    area: Rect,
    title: &str,
    field: &TextField,
    active: bool,
) {
    let border = if active { ACCENT } else { MUTED };
    frame.render_widget(
        Paragraph::new(field.display_value()).block(
            Block::bordered()
                .title(title)
                .border_style(Style::default().fg(border)),
        ),
        area,
    );
    if active && area.width > 2 {
        let width = Line::from(field.displayed_prefix()).width() as u16;
        frame.set_cursor_position(Position::new(
            (area.x + 1 + width).min(area.right().saturating_sub(2)),
            area.y + 1,
        ));
    }
}

pub(super) fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width.saturating_sub(2));
    let height = height.min(area.height.saturating_sub(2));
    let vertical = Layout::new(
        Direction::Vertical,
        [
            Constraint::Fill(1),
            Constraint::Length(height),
            Constraint::Fill(1),
        ],
    )
    .split(area);
    Layout::new(
        Direction::Horizontal,
        [
            Constraint::Fill(1),
            Constraint::Length(width),
            Constraint::Fill(1),
        ],
    )
    .split(vertical[1])[1]
}

pub(super) fn clean(value: &str) -> String {
    value
        .chars()
        .filter(|character| !character.is_control() || matches!(character, '\n' | '\t'))
        .collect()
}
