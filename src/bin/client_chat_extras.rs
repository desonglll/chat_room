//! TG-1205: the parts of a `broadcast` frame that describe a message's context rather than its
//! body — what it replies to (with a quoted snippet or a source in another chat), its emoji
//! reactions, and whether it was sent silently — plus the typing-action phrases. Decoded from the
//! same fields the Web client reads; anything absent stays absent.

use serde::Deserialize;
use uuid::Uuid;

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct MessageExtras {
    #[serde(default)]
    pub reply_to: Option<ReplyPreview>,
    #[serde(default)]
    pub reactions: Vec<Reaction>,
    #[serde(default)]
    pub silent: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ReplyPreview {
    pub message_id: Uuid,
    #[serde(default)]
    pub sender: String,
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub attachment_file_name: Option<String>,
    #[serde(default)]
    pub recalled: bool,
    #[serde(default)]
    pub quote: Option<Quote>,
    /// Set for a reply to a message in another chat.
    #[serde(default)]
    pub chat_title: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Quote {
    pub text: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Reaction {
    pub emoji: String,
    #[serde(default)]
    pub user_ids: Vec<Uuid>,
}

const SNIPPET_CHARS: usize = 48;

impl MessageExtras {
    /// `↳ alice: the original` — the quote replaces the original text when there is one, and a
    /// cross-chat source names its chat (`↳ alice in Team: …`).
    pub fn reply_line(&self) -> Option<String> {
        let reply = self.reply_to.as_ref()?;
        let source = match &reply.chat_title {
            Some(title) => format!("{} in {}", one_line(&reply.sender), one_line(title)),
            None => one_line(&reply.sender),
        };
        let body = if reply.recalled {
            "message recalled".to_string()
        } else if let Some(quote) = &reply.quote {
            format!("“{}”", snippet(&quote.text))
        } else if !reply.content.trim().is_empty() {
            snippet(&reply.content)
        } else if let Some(file) = &reply.attachment_file_name {
            format!("[file] {}", one_line(file))
        } else {
            "message".to_string()
        };
        Some(format!("↳ {source}: {body}"))
    }

    /// `👍 2  ❤ 1`, empty reactions dropped; `None` when there is nothing to show.
    pub fn reaction_line(&self) -> Option<String> {
        let parts = self
            .reactions
            .iter()
            .filter(|reaction| !reaction.user_ids.is_empty())
            .map(|reaction| format!("{} {}", one_line(&reaction.emoji), reaction.user_ids.len()))
            .collect::<Vec<_>>();
        (!parts.is_empty()).then(|| parts.join("  "))
    }

    /// Applies a `reaction_changed` frame. Without a `user_id` (an older server) the change
    /// cannot be attributed, so the counts are left alone.
    pub fn apply_reaction(&mut self, emoji: &str, user_id: Option<Uuid>, active: bool) {
        let Some(user_id) = user_id else { return };
        let index = match self.reactions.iter().position(|r| r.emoji == emoji) {
            Some(index) => index,
            None if active => {
                self.reactions.push(Reaction {
                    emoji: emoji.to_string(),
                    user_ids: Vec::new(),
                });
                self.reactions.len() - 1
            }
            None => return,
        };
        let users = &mut self.reactions[index].user_ids;
        users.retain(|id| *id != user_id);
        if active {
            users.push(user_id);
        }
        if users.is_empty() {
            self.reactions.remove(index);
        }
    }
}

/// The phrase after the name in the typing line, per the server's `typing.action`.
pub fn typing_phrase(action: &str) -> &'static str {
    match action {
        "recording_voice" => "is recording a voice message",
        "recording_video_note" => "is recording a video message",
        "uploading_photo" => "is sending a photo",
        "uploading_video" => "is sending a video",
        "uploading_document" => "is sending a file",
        "uploading_voice" => "is sending a voice message",
        "choosing_sticker" => "is choosing a sticker",
        "choosing_location" => "is choosing a location",
        _ => "is typing",
    }
}

fn one_line(value: &str) -> String {
    value
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect()
}

fn snippet(value: &str) -> String {
    let flat = one_line(value);
    let flat = flat.trim();
    if flat.chars().count() <= SNIPPET_CHARS {
        return flat.to_string();
    }
    let cut: String = flat.chars().take(SNIPPET_CHARS - 1).collect();
    format!("{}…", cut.trim_end())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reply(json: serde_json::Value) -> MessageExtras {
        serde_json::from_value(serde_json::json!({ "reply_to": json })).unwrap()
    }

    #[test]
    fn reply_lines_show_quote_cross_chat_source_and_recall() {
        let id = Uuid::new_v4();
        let plain = reply(serde_json::json!({ "message_id": id, "sender": "alice",
            "content": "line one\nline two", "attachment_file_name": null, "recalled": false }));
        assert_eq!(plain.reply_line().unwrap(), "↳ alice: line one line two");
        let quoted = reply(serde_json::json!({ "message_id": id, "sender": "alice",
            "content": "the whole thing", "recalled": false, "quote": { "text": "whole", "offset": 4 } }));
        assert_eq!(quoted.reply_line().unwrap(), "↳ alice: “whole”");
        let cross = reply(
            serde_json::json!({ "message_id": id, "sender": "bob", "content": "hi",
            "recalled": false, "chat_id": Uuid::new_v4(), "chat_title": "Team" }),
        );
        assert_eq!(cross.reply_line().unwrap(), "↳ bob in Team: hi");
        let gone = reply(
            serde_json::json!({ "message_id": id, "sender": "bob", "content": "",
            "recalled": true }),
        );
        assert_eq!(gone.reply_line().unwrap(), "↳ bob: message recalled");
        let file = reply(
            serde_json::json!({ "message_id": id, "sender": "bob", "content": "",
            "attachment_file_name": "a.pdf", "recalled": false }),
        );
        assert_eq!(file.reply_line().unwrap(), "↳ bob: [file] a.pdf");
        let long = reply(serde_json::json!({ "message_id": id, "sender": "a",
            "content": "x".repeat(100), "recalled": false }));
        assert_eq!(
            long.reply_line().unwrap().chars().count(),
            "↳ a: ".chars().count() + SNIPPET_CHARS
        );
        assert!(MessageExtras::default().reply_line().is_none());
    }

    #[test]
    fn reactions_count_and_toggle_per_user() {
        let (alice, bob) = (Uuid::new_v4(), Uuid::new_v4());
        let mut extras: MessageExtras = serde_json::from_value(serde_json::json!({
            "reactions": [{ "emoji": "👍", "user_ids": [alice] }] }))
        .unwrap();
        assert_eq!(extras.reaction_line().unwrap(), "👍 1");
        extras.apply_reaction("👍", Some(bob), true);
        extras.apply_reaction("👍", Some(bob), true); // replayed frame: no double count
        extras.apply_reaction("❤", Some(alice), true);
        assert_eq!(extras.reaction_line().unwrap(), "👍 2  ❤ 1");
        extras.apply_reaction("👍", Some(alice), false);
        extras.apply_reaction("❤", Some(alice), false);
        extras.apply_reaction("🔥", Some(alice), false);
        extras.apply_reaction("👍", None, false);
        assert_eq!(extras.reaction_line().unwrap(), "👍 1");
        extras.apply_reaction("👍", Some(bob), false);
        assert!(extras.reaction_line().is_none());
    }

    #[test]
    fn typing_actions_have_phrases() {
        assert_eq!(
            typing_phrase("recording_voice"),
            "is recording a voice message"
        );
        assert_eq!(typing_phrase("choosing_sticker"), "is choosing a sticker");
        assert_eq!(typing_phrase("typing"), "is typing");
        assert_eq!(typing_phrase("from_the_future"), "is typing");
    }
}
