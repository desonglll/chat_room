//! The message entity type and its pure validation rules.
//!
//! Offsets and lengths are UTF-16 code units of the message text — what JavaScript string
//! indices and Telegram use — so a client never converts. Validation never refuses a
//! message: an invalid entity is dropped and the text is kept.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

/// Most entities one message may carry (Telegram's own ceiling).
pub const MAX_ENTITIES: usize = 100;
const MAX_URL_CHARS: usize = 2048;
const MAX_LANGUAGE_CHARS: usize = 32;

pub const CUSTOM_EMOJI: &str = "custom_emoji";
pub const TEXT_LINK: &str = "text_link";
pub const MENTION_NAME: &str = "mention_name";
pub const PRE: &str = "pre";

/// Every entity type the server stores. Only `custom_emoji` is rendered today; the formatting
/// types are accepted and persisted so a later task only has to draw them.
pub const ENTITY_TYPES: [&str; 10] = [
    CUSTOM_EMOJI,
    "bold",
    "italic",
    "underline",
    "strikethrough",
    "spoiler",
    "code",
    PRE,
    TEXT_LINK,
    MENTION_NAME,
];

/// One formatted range of a message's text. The type-specific fields are present only for
/// their type (`custom_emoji_id`, `url` for `text_link`, `user_id` for `mention_name`,
/// `language` for `pre`) and omitted otherwise.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct MessageEntity {
    #[serde(rename = "type")]
    pub entity_type: String,
    pub offset: i64,
    pub length: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_emoji_id: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_id: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
}

impl MessageEntity {
    pub fn custom_emoji(offset: i64, length: i64, id: Uuid) -> Self {
        MessageEntity {
            entity_type: CUSTOM_EMOJI.to_string(),
            offset,
            length,
            custom_emoji_id: Some(id),
            url: None,
            user_id: None,
            language: None,
        }
    }

    pub fn is_custom_emoji(&self) -> bool {
        self.entity_type == CUSTOM_EMOJI
    }

    fn end(&self) -> i64 {
        self.offset + self.length
    }

    /// Keep only the field this type uses; `None` when the type or its field is invalid.
    fn normalized(mut self) -> Option<Self> {
        if !ENTITY_TYPES.contains(&self.entity_type.as_str()) {
            return None;
        }
        let kind = self.entity_type.as_str();
        if kind != CUSTOM_EMOJI {
            self.custom_emoji_id = None;
        }
        if kind != TEXT_LINK {
            self.url = None;
        }
        if kind != MENTION_NAME {
            self.user_id = None;
        }
        if kind != PRE {
            self.language = None;
        }
        let valid = match kind {
            CUSTOM_EMOJI => self.custom_emoji_id.is_some(),
            TEXT_LINK => self.url.as_deref().is_some_and(valid_link),
            MENTION_NAME => self.user_id.is_some(),
            PRE => self
                .language
                .as_deref()
                .is_none_or(|language| language.chars().count() <= MAX_LANGUAGE_CHARS),
            _ => true,
        };
        valid.then_some(self)
    }
}

fn valid_link(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    (lower.starts_with("https://") || lower.starts_with("http://"))
        && url.chars().count() <= MAX_URL_CHARS
        && !url.chars().any(char::is_control)
}

/// Length of `text` in UTF-16 code units.
pub fn utf16_len(text: &str) -> i64 {
    text.encode_utf16().count() as i64
}

/// UTF-16 units the server's `trim()` removes from the front of `raw`, so entities computed
/// against the raw text can be shifted onto the stored (trimmed) text.
pub fn leading_trim_utf16(raw: &str) -> i64 {
    utf16_len(&raw[..raw.len() - raw.trim_start().len()])
}

/// Move entities computed against untrimmed text onto the trimmed text.
pub fn shift_entities(entities: Vec<MessageEntity>, by: i64) -> Vec<MessageEntity> {
    entities
        .into_iter()
        .map(|mut entity| {
            entity.offset -= by;
            entity
        })
        .collect()
}

/// Every UTF-16 index of `text` that falls between two characters (never inside a
/// surrogate pair), as a lookup table of `utf16_len + 1` entries.
fn char_boundaries(text: &str) -> Vec<bool> {
    let mut boundaries = vec![false; text.encode_utf16().count() + 1];
    let mut index = 0;
    boundaries[0] = true;
    for character in text.chars() {
        index += character.len_utf16();
        boundaries[index] = true;
    }
    boundaries
}

/// The valid subset of `entities` for `content`, sorted by offset: known type with its
/// field, non-empty, inside the text, on character boundaries, custom emoji never
/// overlapping each other, at most [`MAX_ENTITIES`]. Pure; existence of the referenced
/// custom emoji is checked by the caller against the database.
pub fn sanitize_entities(content: &str, entities: Vec<MessageEntity>) -> Vec<MessageEntity> {
    let boundaries = char_boundaries(content);
    let text_len = boundaries.len() as i64 - 1;
    let mut valid: Vec<MessageEntity> = entities
        .into_iter()
        .filter(|entity| {
            entity.offset >= 0
                && entity.length > 0
                && entity.end() <= text_len
                && boundaries[entity.offset as usize]
                && boundaries[entity.end() as usize]
        })
        .filter_map(MessageEntity::normalized)
        .collect();
    // Outer ranges before the ranges they contain; ties keep the client's order.
    valid.sort_by_key(|entity| (entity.offset, -entity.length));
    let mut kept: Vec<MessageEntity> = Vec::with_capacity(valid.len());
    let mut emoji_end = 0;
    for entity in valid {
        if entity.is_custom_emoji() {
            if entity.offset < emoji_end {
                continue;
            }
            emoji_end = entity.end();
        }
        if kept.len() == MAX_ENTITIES {
            break;
        }
        kept.push(entity);
    }
    kept
}

#[cfg(test)]
mod tests {
    use super::*;

    fn emoji(offset: i64, length: i64) -> MessageEntity {
        MessageEntity::custom_emoji(offset, length, Uuid::nil())
    }

    #[test]
    fn utf16_offsets_count_surrogate_pairs_as_two() {
        assert_eq!(utf16_len("a😀"), 3);
        assert_eq!(leading_trim_utf16("  \u{3000}hi "), 3);
        let kept = sanitize_entities("a😀b", vec![emoji(1, 2), emoji(1, 1), emoji(3, 2)]);
        assert_eq!(
            kept,
            vec![emoji(1, 2)],
            "inside a pair or past the end is dropped"
        );
    }

    #[test]
    fn overlapping_custom_emoji_keep_the_first_and_unknown_types_drop() {
        let mut bold = emoji(0, 4);
        bold.entity_type = "bold".into();
        let mut unknown = emoji(0, 1);
        unknown.entity_type = "blink".into();
        let kept = sanitize_entities("😀😀", vec![emoji(2, 2), emoji(0, 4), bold, unknown]);
        assert_eq!(kept.len(), 2);
        assert_eq!(kept[0], emoji(0, 4));
        assert_eq!(kept[1].entity_type, "bold");
        assert_eq!(kept[1].custom_emoji_id, None, "foreign fields are stripped");
    }

    #[test]
    fn links_must_be_http_and_the_list_is_capped() {
        let mut link = emoji(0, 1);
        link.entity_type = TEXT_LINK.into();
        link.url = Some("javascript:alert(1)".into());
        assert!(sanitize_entities("x", vec![link]).is_empty());
        let text = "😀".repeat(150);
        let many = (0..150).map(|i| emoji(i * 2, 2)).collect();
        assert_eq!(sanitize_entities(&text, many).len(), MAX_ENTITIES);
    }
}
