//! TG-907: the terminal client's share of Telegram parity — the Contacts tab (friends with
//! presence, incoming requests), the open chat's pinned messages, and forwarding. State and
//! contracts live here; `dispatch_social` runs the requests, `update_social` handles keys and
//! answers, `render_social` draws.

use std::collections::HashMap;

use uuid::Uuid;

use crate::client_api::ApiResult;
use crate::client_api_social::{status_text, ChatPin, ForwardResult, Friend, FriendRequest};

#[derive(Clone, Debug)]
pub enum SocialAction {
    LoadContacts,
    Respond {
        user_id: Uuid,
        accept: bool,
    },
    AddContact(String),
    OpenDirectChat(Uuid),
    LoadPins(Uuid),
    SetPinned {
        room_id: Uuid,
        message_id: Uuid,
        pinned: bool,
    },
    Forward {
        message_id: Uuid,
        target: Uuid,
        title: String,
    },
    /// TG-1103: option indexes (0-based); empty retracts.
    Vote {
        message_id: Uuid,
        options: Vec<u32>,
    },
    LoadFolders,
}

#[derive(Debug)]
pub enum SocialEvent {
    Contacts(ApiResult<ContactsData>),
    /// A contacts change finished; the text is the status line, then contacts reload.
    ContactsChanged(ApiResult<String>),
    DirectChatOpened(ApiResult<Uuid>),
    Pins {
        room_id: Uuid,
        result: ApiResult<Vec<ChatPin>>,
    },
    PinChanged {
        room_id: Uuid,
        pinned: bool,
        result: ApiResult<()>,
    },
    Forwarded {
        title: String,
        result: ApiResult<ForwardResult>,
    },
    Voted {
        message_id: Uuid,
        result: ApiResult<crate::client_chat_media::Poll>,
    },
    Folders(ApiResult<Vec<crate::client_api_polls_folders::ChatFolder>>),
}

#[derive(Debug, Default)]
pub struct ContactsData {
    pub friends: Vec<Friend>,
    pub statuses: HashMap<Uuid, serde_json::Value>,
    pub requests: Vec<FriendRequest>,
}

#[derive(Debug, Default)]
pub struct SocialState {
    pub contacts: ContactsData,
    pub index: usize,
    /// Pinned messages of the open chat, newest first (`(message id, one line)`).
    pub pins: Vec<(Uuid, String)>,
    /// TG-1103: the account's chat folders, the one filtering the list (`None` = all chats),
    /// and the unfiltered conversation list the filter is applied to.
    pub folders: Vec<crate::client_api_polls_folders::ChatFolder>,
    pub folder: Option<usize>,
    pub all_conversations: Vec<crate::client_api::Conversation>,
}

impl SocialState {
    /// The conversations the list shows under the current folder.
    pub fn visible_conversations(&self) -> Vec<crate::client_api::Conversation> {
        let now = chrono::Utc::now();
        match self.folder.and_then(|index| self.folders.get(index)) {
            None => self.all_conversations.clone(),
            Some(folder) => self
                .all_conversations
                .iter()
                .filter(|chat| folder.contains(chat, now))
                .cloned()
                .collect(),
        }
    }

    pub fn folder_title(&self) -> Option<String> {
        self.folder
            .and_then(|index| self.folders.get(index))
            .map(|folder| {
                format!("{} {}", folder.emoji, folder.title)
                    .trim()
                    .to_string()
            })
    }
}

/// `"1, 3"` → `[0, 2]` (1-based input, de-duplicated); `None` when anything is not a number in
/// `1..=count`. An empty input is an empty list (retract).
pub fn parse_vote(input: &str, count: usize) -> Option<Vec<u32>> {
    let mut options = Vec::new();
    for part in input
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|p| !p.is_empty())
    {
        let number: usize = part.parse().ok()?;
        if number == 0 || number > count {
            return None;
        }
        let index = (number - 1) as u32;
        if !options.contains(&index) {
            options.push(index);
        }
    }
    Some(options)
}

/// One row of the Contacts tab: requests first, then friends online-first.
#[derive(Clone, Debug, PartialEq)]
pub enum ContactRow {
    Request {
        user_id: Uuid,
        label: String,
    },
    Friend {
        user_id: Uuid,
        name: String,
        status: String,
        online: bool,
    },
}

fn status_rank(status: Option<&serde_json::Value>) -> (u8, i64) {
    let kind = status
        .and_then(|status| status["kind"].as_str())
        .unwrap_or("");
    let seen = status
        .and_then(|status| status["last_seen"].as_str())
        .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
        .map(|seen| -seen.timestamp())
        .unwrap_or(0);
    let rank = match kind {
        "online" => 0,
        "offline" => 1,
        "recently" => 2,
        "within_week" => 3,
        "within_month" => 4,
        _ => 5,
    };
    (rank, seen)
}

impl ContactsData {
    pub fn rows(&self) -> Vec<ContactRow> {
        let mut rows: Vec<ContactRow> = self
            .requests
            .iter()
            .map(|request| ContactRow::Request {
                user_id: request.user.id,
                label: if request.user.display_name.trim().is_empty() {
                    format!("@{}", request.user.username)
                } else {
                    format!("{} (@{})", request.user.display_name, request.user.username)
                },
            })
            .collect();
        let mut friends: Vec<&Friend> = self.friends.iter().collect();
        friends.sort_by_key(|friend| {
            let (rank, seen) = status_rank(self.statuses.get(&friend.id));
            (rank, seen, friend.name().to_lowercase())
        });
        rows.extend(friends.into_iter().map(|friend| {
            let status = self.statuses.get(&friend.id);
            ContactRow::Friend {
                user_id: friend.id,
                name: friend.name(),
                status: status_text(status),
                online: status.and_then(|status| status["kind"].as_str()) == Some("online"),
            }
        }));
        rows
    }
}

/// Pins as the chat header line shows them: newest first, recalled ones dropped.
pub fn pin_lines(mut pins: Vec<ChatPin>) -> Vec<(Uuid, String)> {
    pins.retain(|pin| pin.message.recalled_at.is_none());
    pins.sort_by(|a, b| b.pinned_at.cmp(&a.pinned_at));
    pins.into_iter()
        .map(|pin| {
            let text = pin
                .message
                .content
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
            (
                pin.message.id,
                if text.is_empty() {
                    "[media]".into()
                } else {
                    text
                },
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vote_input_is_one_based_and_validated() {
        assert_eq!(parse_vote("1", 3), Some(vec![0]));
        assert_eq!(parse_vote("1, 3 3", 3), Some(vec![0, 2]));
        assert_eq!(parse_vote("", 3), Some(vec![]));
        assert_eq!(parse_vote("4", 3), None);
        assert_eq!(parse_vote("x", 3), None);
    }
    use crate::client_api_social::{PinnedMessage, RequestUser};

    fn friend(id: u128, username: &str, remark: &str) -> Friend {
        Friend {
            id: Uuid::from_u128(id),
            username: username.into(),
            display_name: String::new(),
            remark: remark.into(),
        }
    }

    #[test]
    fn requests_come_first_then_friends_online_first_then_most_recent() {
        let mut data = ContactsData {
            friends: vec![
                friend(1, "ann", ""),
                friend(2, "bob", "老王"),
                friend(3, "cat", ""),
                friend(4, "dan", ""),
            ],
            requests: vec![FriendRequest {
                user: RequestUser {
                    id: Uuid::from_u128(9),
                    username: "eve".into(),
                    display_name: String::new(),
                },
            }],
            ..ContactsData::default()
        };
        data.statuses.insert(
            Uuid::from_u128(1),
            serde_json::json!({"kind": "offline", "last_seen": "2026-10-01T08:00:00Z"}),
        );
        data.statuses
            .insert(Uuid::from_u128(2), serde_json::json!({"kind": "online"}));
        data.statuses.insert(
            Uuid::from_u128(3),
            serde_json::json!({"kind": "offline", "last_seen": "2026-10-01T09:00:00Z"}),
        );
        let rows = data.rows();
        assert_eq!(
            rows[0],
            ContactRow::Request {
                user_id: Uuid::from_u128(9),
                label: "@eve".into()
            }
        );
        let names: Vec<String> = rows[1..]
            .iter()
            .map(|row| match row {
                ContactRow::Friend { name, .. } => name.clone(),
                ContactRow::Request { .. } => unreachable!(),
            })
            .collect();
        assert_eq!(names, ["老王", "cat", "ann", "dan"]);
        assert!(
            matches!(&rows[1], ContactRow::Friend { online: true, status, .. } if status == "online")
        );
    }

    #[test]
    fn pin_lines_are_newest_first_and_skip_recalled() {
        let pin = |id: u128, content: &str, at: &str, recalled: bool| ChatPin {
            message: PinnedMessage {
                id: Uuid::from_u128(id),
                content: content.into(),
                recalled_at: recalled.then(|| "x".into()),
            },
            pinned_at: at.into(),
        };
        let lines = pin_lines(vec![
            pin(1, "old\n note", "2026-10-01T01:00:00Z", false),
            pin(2, "new", "2026-10-01T02:00:00Z", false),
            pin(3, "gone", "2026-10-01T03:00:00Z", true),
            pin(4, "", "2026-10-01T00:00:00Z", false),
        ]);
        assert_eq!(
            lines,
            [
                (Uuid::from_u128(2), "new".to_string()),
                (Uuid::from_u128(1), "old note".to_string()),
                (Uuid::from_u128(4), "[media]".to_string()),
            ]
        );
    }
}
