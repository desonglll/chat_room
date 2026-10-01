//! TG-1210: chat titles may repeat (as in Telegram), so `join --room-name` resolves a title
//! only when exactly one visible chat carries it, and otherwise names the candidates.

use anyhow::{bail, Result};
use uuid::Uuid;

use crate::client_api_models::ChatSummary;

/// `chats` may list one chat twice (joined *and* discoverable); it counts once.
pub fn resolve_chat_title(chats: Vec<ChatSummary>, name: &str) -> Result<Option<Uuid>> {
    let mut matches: Vec<ChatSummary> = Vec::new();
    for chat in chats.into_iter().filter(|chat| chat.title == name) {
        if !matches.iter().any(|seen| seen.id == chat.id) {
            matches.push(chat);
        }
    }
    match matches.as_slice() {
        [] => Ok(None),
        [only] => Ok(Some(only.id)),
        several => bail!(
            "{} chats are named '{name}'; join one with --room-id:\n{}",
            several.len(),
            several
                .iter()
                .map(candidate_line)
                .collect::<Vec<_>>()
                .join("\n")
        ),
    }
}

fn candidate_line(chat: &ChatSummary) -> String {
    let members = chat.member_count.map_or_else(
        || "? members".to_string(),
        |count| format!("{count} members"),
    );
    let joined = if chat.membership_status.as_deref() == Some("active") {
        ", joined"
    } else {
        ""
    };
    format!("  {}  ({members}{joined})", chat.id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chat(id: Uuid, title: &str, members: Option<i64>, status: Option<&str>) -> ChatSummary {
        ChatSummary {
            id,
            title: title.to_string(),
            chat_type: Some("group".to_string()),
            has_password: false,
            membership_status: status.map(str::to_string),
            member_count: members,
        }
    }

    #[test]
    fn a_unique_title_resolves_and_a_missing_one_is_none() {
        let id = Uuid::new_v4();
        let chats = vec![
            chat(id, "lobby", Some(3), None),
            chat(Uuid::new_v4(), "x", None, None),
        ];
        assert_eq!(
            resolve_chat_title(chats.clone(), "lobby").unwrap(),
            Some(id)
        );
        assert_eq!(resolve_chat_title(chats, "nope").unwrap(), None);
    }

    #[test]
    fn the_same_chat_listed_twice_is_not_ambiguous() {
        let id = Uuid::new_v4();
        let chats = vec![
            chat(id, "lobby", Some(3), Some("active")),
            chat(id, "lobby", Some(3), None),
        ];
        assert_eq!(resolve_chat_title(chats, "lobby").unwrap(), Some(id));
    }

    #[test]
    fn a_repeated_title_errors_and_lists_every_candidate() {
        let (first, second) = (Uuid::new_v4(), Uuid::new_v4());
        let chats = vec![
            chat(first, "lobby", Some(12), Some("active")),
            chat(second, "lobby", None, None),
        ];
        let message = resolve_chat_title(chats, "lobby").unwrap_err().to_string();
        assert!(message.contains("2 chats are named 'lobby'"), "{message}");
        assert!(message.contains("--room-id"), "{message}");
        assert!(
            message.contains(&format!("{first}  (12 members, joined)")),
            "{message}"
        );
        assert!(
            message.contains(&format!("{second}  (? members)")),
            "{message}"
        );
    }
}
