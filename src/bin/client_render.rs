//! Terminal-safe rendering for interactive chat output.

use colored::Colorize;

use crate::client_media::{self, Attachment};

pub fn chat(
    sender: &str,
    content: &str,
    attachment: Option<&Attachment>,
    timestamp: &str,
    current_user: &str,
    http_base: &str,
) -> String {
    let time = short_time(timestamp);
    let safe_sender = sanitize(sender);
    let mut safe_content = sanitize(content);
    if let Some(attachment) = attachment {
        if !safe_content.is_empty() {
            safe_content.push('\n');
        }
        safe_content.push_str(&client_media::render(attachment, http_base));
    }
    if sender == current_user {
        format!(
            "{} {} {}",
            time.dimmed(),
            "[you]".green().bold(),
            safe_content
        )
    } else {
        format!(
            "{} {} {}",
            time.dimmed(),
            format!("[{safe_sender}]").blue().bold(),
            safe_content
        )
    }
}

pub fn system(content: &str) -> String {
    format!(
        "{} {}",
        "[system]".cyan().bold(),
        sanitize(content).dimmed()
    )
}

pub fn error(content: &str) -> String {
    format!("{} {}", "[error]".red().bold(), sanitize(content))
}

fn short_time(timestamp: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(timestamp)
        .map(|value| {
            value
                .with_timezone(&chrono::Local)
                .format("%H:%M:%S")
                .to_string()
        })
        .unwrap_or_else(|_| sanitize(timestamp))
}

fn sanitize(value: &str) -> String {
    value
        .chars()
        .filter(|character| !character.is_control())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn separates_message_types() {
        let own = chat(
            "alice",
            "hello",
            None,
            "2026-08-14T06:00:00Z",
            "alice",
            "http://localhost",
        );
        let other = chat(
            "bob",
            "hi",
            None,
            "2026-08-14T06:00:01Z",
            "alice",
            "http://localhost",
        );
        assert!(own.contains("[you]"));
        assert!(other.contains("[bob]"));
        assert!(system("bob joined").contains("[system]"));
    }

    #[test]
    fn removes_terminal_control_characters() {
        assert_eq!(sanitize("hello\u{1b}[2J\nworld"), "hello[2Jworld");
    }
}
