//! TG-1103: the parts of a chat message a terminal can show as text — polls (with results),
//! voice and round-video messages, stickers, GIFs, locations and contact cards. Decoded from the
//! same `broadcast` frame fields the Web client reads; anything absent stays absent.

use serde::Deserialize;

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct MessageMedia {
    #[serde(default)]
    pub media_kind: Option<String>,
    #[serde(default)]
    pub poll: Option<Poll>,
    #[serde(default)]
    pub voice: Option<Clip>,
    #[serde(default)]
    pub video_note: Option<Clip>,
    #[serde(default)]
    pub sticker: Option<Sticker>,
    #[serde(default)]
    pub location: Option<Location>,
    #[serde(default)]
    pub contact: Option<ContactCard>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Poll {
    pub question: String,
    #[serde(default)]
    pub closed: bool,
    #[serde(default)]
    pub total_voters: i64,
    pub options: Vec<PollOption>,
    #[serde(default)]
    pub multiple_choice: bool,
    #[serde(default)]
    pub quiz: bool,
    /// The viewer's own choices; absent in chat-wide frames.
    #[serde(default)]
    pub chosen: Option<Vec<u32>>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct PollOption {
    pub text: String,
    #[serde(default)]
    pub voters: i64,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Clip {
    pub duration_ms: u32,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Sticker {
    #[serde(default)]
    pub emoji: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Location {
    pub latitude: f64,
    pub longitude: f64,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub live_until: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ContactCard {
    pub username: String,
    #[serde(default)]
    pub display_name: String,
}

fn clock(duration_ms: u32) -> String {
    let seconds = duration_ms.div_ceil(1000);
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

impl Poll {
    /// `Lunch? (2 votes, closed)` then one line per option: `1. [x] Yes  67% (2)`.
    pub fn lines(&self) -> Vec<String> {
        let mut flags = vec![format!(
            "{} vote{}",
            self.total_voters,
            if self.total_voters == 1 { "" } else { "s" }
        )];
        if self.quiz {
            flags.push("quiz".into());
        }
        if self.multiple_choice {
            flags.push("multiple choice".into());
        }
        if self.closed {
            flags.push("closed".into());
        }
        let mut lines = vec![format!("📊 {} ({})", self.question, flags.join(", "))];
        let voted = self
            .chosen
            .as_ref()
            .is_some_and(|chosen| !chosen.is_empty());
        for (index, option) in self.options.iter().enumerate() {
            let mine = self
                .chosen
                .as_ref()
                .is_some_and(|chosen| chosen.contains(&(index as u32)));
            let result = if voted || self.closed {
                let percent = if self.total_voters > 0 {
                    option.voters * 100 / self.total_voters
                } else {
                    0
                };
                format!("  {percent}% ({})", option.voters)
            } else {
                String::new()
            };
            lines.push(format!(
                "{}. [{}] {}{result}",
                index + 1,
                if mine { "x" } else { " " },
                option.text
            ));
        }
        lines
    }
}

impl MessageMedia {
    /// One label line for non-poll media (`[Voice message 0:04]` …); `None` for plain text.
    pub fn label(&self) -> Option<String> {
        if let Some(voice) = &self.voice {
            return Some(format!("[Voice message {}]", clock(voice.duration_ms)));
        }
        if let Some(note) = &self.video_note {
            return Some(format!("[Video message {}]", clock(note.duration_ms)));
        }
        if let Some(sticker) = &self.sticker {
            return Some(format!("[Sticker {}]", sticker.emoji).replace(" ]", "]"));
        }
        if let Some(location) = &self.location {
            let kind = if location.live_until.is_some() {
                "Live location"
            } else {
                "Location"
            };
            let place = if location.title.is_empty() {
                format!("{:.5}, {:.5}", location.latitude, location.longitude)
            } else {
                location.title.clone()
            };
            return Some(format!("[{kind}] {place}"));
        }
        if let Some(contact) = &self.contact {
            let name = if contact.display_name.is_empty() {
                format!("@{}", contact.username)
            } else {
                format!("{} (@{})", contact.display_name, contact.username)
            };
            return Some(format!("[Contact] {name}"));
        }
        match self.media_kind.as_deref() {
            Some("gif") => Some("[GIF]".into()),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decode(value: serde_json::Value) -> MessageMedia {
        serde_json::from_value(value).unwrap()
    }

    #[test]
    fn media_labels_read_like_telegram_previews() {
        assert_eq!(decode(serde_json::json!({})).label(), None);
        let voice = decode(
            serde_json::json!({ "media_kind": "voice", "voice": { "duration_ms": 3480, "waveform": [] } }),
        );
        assert_eq!(voice.label().unwrap(), "[Voice message 0:04]");
        let note = decode(serde_json::json!({ "video_note": { "duration_ms": 61_000 } }));
        assert_eq!(note.label().unwrap(), "[Video message 1:01]");
        let sticker =
            decode(serde_json::json!({ "sticker": { "emoji": "😀", "sticker_id": "x" } }));
        assert_eq!(sticker.label().unwrap(), "[Sticker 😀]");
        let live = decode(
            serde_json::json!({ "location": { "latitude": 1.0, "longitude": 2.0, "live_until": "t" } }),
        );
        assert_eq!(live.label().unwrap(), "[Live location] 1.00000, 2.00000");
        let contact =
            decode(serde_json::json!({ "contact": { "username": "bob", "display_name": "Bob" } }));
        assert_eq!(contact.label().unwrap(), "[Contact] Bob (@bob)");
        assert_eq!(
            decode(serde_json::json!({ "media_kind": "gif" }))
                .label()
                .unwrap(),
            "[GIF]"
        );
    }

    #[test]
    fn a_poll_shows_results_after_voting_or_closing() {
        let mut poll = Poll {
            question: "Lunch?".into(),
            closed: false,
            total_voters: 3,
            options: vec![
                PollOption {
                    text: "Yes".into(),
                    voters: 2,
                },
                PollOption {
                    text: "No".into(),
                    voters: 1,
                },
            ],
            multiple_choice: false,
            quiz: false,
            chosen: None,
        };
        assert_eq!(
            poll.lines(),
            ["📊 Lunch? (3 votes)", "1. [ ] Yes", "2. [ ] No"]
        );
        poll.chosen = Some(vec![0]);
        assert_eq!(poll.lines()[1], "1. [x] Yes  66% (2)");
        assert_eq!(poll.lines()[2], "2. [ ] No  33% (1)");
    }
}
