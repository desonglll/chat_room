//! TG-409: quoting part of a replied-to message, and replying to a message in another chat.
//!
//! A quote must really be a slice of the original: the server checks it against the
//! original's text (UTF-16 offsets, like TG-304 entities) and drops a quote that does not
//! match rather than refusing the message. A cross-chat reply is allowed only when the sender
//! can read the source chat; the target chat then sees a snapshot the sender chose to share —
//! the source's sender name, chat title, and the quote (or the first 200 characters) — never
//! the live source row, because its members may have no access to the source chat.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::chats::chat_type::ChatType;
use chrono::{DateTime, Utc};
use sqlx::FromRow;

use crate::models::{ReplyPreview, ReplyQuote};
use crate::state::{with_pool, AppState};

/// Longest quote kept (UTF-16 code units), and the snapshot of an unquoted cross-chat reply.
pub const MAX_QUOTE_UTF16: usize = 1024;
const SNAPSHOT_CHARS: usize = 200;

/// The client's request: the quoted text and where it starts in the original.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ReplyQuoteRequest {
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset: Option<i64>,
}

/// A reply into another chat, as the target chat will see it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrossChatReply {
    pub message_id: Uuid,
    pub chat_id: Uuid,
    pub sender: String,
    pub chat_title: String,
    /// The quote's text when there is a quote, else the source's first 200 characters.
    pub snapshot: String,
}

/// Everything a reply stores beyond `reply_to_id`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReplyExtra {
    pub quote: Option<ReplyQuote>,
    pub cross: Option<CrossChatReply>,
}

/// Where `quote` sits in `original` (UTF-16 offset): at the requested offset when it matches
/// there, else its first occurrence; `None` when it is empty, too long, or not a slice.
pub fn locate_quote(original: &str, quote: &str, requested_offset: Option<i64>) -> Option<i64> {
    let haystack: Vec<u16> = original.encode_utf16().collect();
    let needle: Vec<u16> = quote.encode_utf16().collect();
    if needle.is_empty() || needle.len() > MAX_QUOTE_UTF16 || needle.len() > haystack.len() {
        return None;
    }
    let matches_at =
        |start: usize| haystack.get(start..start + needle.len()) == Some(needle.as_slice());
    if let Some(offset) = requested_offset.and_then(|offset| usize::try_from(offset).ok()) {
        if matches_at(offset) {
            return i64::try_from(offset).ok();
        }
    }
    (0..=haystack.len() - needle.len())
        .find(|&start| matches_at(start))
        .and_then(|start| i64::try_from(start).ok())
}

/// The reply columns of `MESSAGE_SELECT` (flattened into `MessageRow`): the live same-chat
/// preview join (`reply.*`, NULL for a cross-chat reply) and the TG-409 quote/snapshot columns.
#[derive(Debug, FromRow)]
pub(crate) struct ReplyColumns {
    reply_message_id: Option<Uuid>,
    reply_sender: Option<String>,
    reply_content: Option<String>,
    reply_recalled_at: Option<DateTime<Utc>>,
    reply_attachment_file_name: Option<String>,
    reply_target_id: Option<Uuid>,
    reply_to_chat_id: Option<Uuid>,
    reply_quote_text: Option<String>,
    reply_quote_offset: Option<i64>,
    reply_source_sender: Option<String>,
    reply_source_chat_title: Option<String>,
    reply_source_edited_at: Option<DateTime<Utc>>,
}

impl ReplyColumns {
    /// The reply preview of a message created at `created_at`: the live same-chat preview, or
    /// a cross-chat snapshot. A snapshot without an offset is not a quote.
    pub(crate) fn into_preview(self, created_at: DateTime<Utc>) -> Option<ReplyPreview> {
        let quote = self
            .reply_quote_offset
            .zip(self.reply_quote_text.clone())
            .map(|(offset, text)| ReplyQuote { text, offset });
        let quote_modified = quote.is_some()
            && self
                .reply_source_edited_at
                .is_some_and(|edited| edited > created_at);
        if let Some(message_id) = self.reply_message_id {
            let recalled = self.reply_recalled_at.is_some();
            return Some(ReplyPreview {
                message_id,
                sender: self.reply_sender?,
                content: if recalled {
                    String::new()
                } else {
                    self.reply_content?
                },
                attachment_file_name: if recalled {
                    None
                } else {
                    self.reply_attachment_file_name
                },
                recalled,
                quote: if recalled { None } else { quote },
                quote_modified,
                ..Default::default()
            });
        }
        let (message_id, chat_id) = self.reply_target_id.zip(self.reply_to_chat_id)?;
        Some(ReplyPreview {
            message_id,
            sender: self.reply_source_sender.unwrap_or_default(),
            content: self.reply_quote_text.unwrap_or_default(),
            attachment_file_name: None,
            recalled: false,
            quote,
            quote_modified,
            chat_id: Some(chat_id),
            chat_title: self.reply_source_chat_title,
        })
    }
}

impl AppState {
    /// [`Self::resolve_reply_extra`] for the WebSocket path: a database error degrades to the
    /// plain same-chat reply (logged) rather than dropping the message.
    pub async fn reply_or_plain(
        &self,
        room_id: Uuid,
        sender_id: Uuid,
        reply_to: Option<Uuid>,
        reply_to_chat_id: Option<Uuid>,
        quote: Option<ReplyQuoteRequest>,
    ) -> (Option<Uuid>, ReplyExtra) {
        self.resolve_reply_extra(room_id, sender_id, reply_to, reply_to_chat_id, quote)
            .await
            .unwrap_or_else(|error| {
                tracing::warn!("resolve reply failed: {error}");
                (reply_to, ReplyExtra::default())
            })
    }

    /// Resolve a reply's extras for `sender_id` posting into `room_id`. Returns the same-chat
    /// `reply_to` to validate as before (unchanged for a cross-chat reply: `None`) and the
    /// extras. A cross-chat reply the sender may not read is dropped entirely (a plain message).
    pub async fn resolve_reply_extra(
        &self,
        room_id: Uuid,
        sender_id: Uuid,
        reply_to: Option<Uuid>,
        reply_to_chat_id: Option<Uuid>,
        quote: Option<ReplyQuoteRequest>,
    ) -> Result<(Option<Uuid>, ReplyExtra), sqlx::Error> {
        let Some(message_id) = reply_to else {
            return Ok((None, ReplyExtra::default()));
        };
        let source_chat = reply_to_chat_id.filter(|chat| *chat != room_id);
        let Some(source_chat) = source_chat else {
            // Same chat: the quote is checked against the original's text.
            let quote = match (quote, self.reply_preview(room_id, Some(message_id)).await?) {
                (Some(quote), Some(original)) if !original.recalled => {
                    locate_quote(&original.content, &quote.text, quote.offset).map(|offset| {
                        ReplyQuote {
                            text: quote.text,
                            offset,
                        }
                    })
                }
                _ => None,
            };
            return Ok((Some(message_id), ReplyExtra { quote, cross: None }));
        };
        if !self.can_read_chat(source_chat, sender_id).await? {
            return Ok((None, ReplyExtra::default()));
        }
        let source: Option<(String, String)> = with_pool!(self, |pool| {
            sqlx::query_as(
                "SELECT sender, content FROM messages \
                 WHERE id = $1 AND room_id = $2 AND recalled_at IS NULL",
            )
            .bind(message_id)
            .bind(source_chat)
            .fetch_optional(pool)
            .await
        })?;
        let Some((sender, content)) = source else {
            return Ok((None, ReplyExtra::default()));
        };
        let quote = quote.and_then(|quote| {
            locate_quote(&content, &quote.text, quote.offset).map(|offset| ReplyQuote {
                text: quote.text,
                offset,
            })
        });
        let chat_title = match self.chat(source_chat).await {
            Some(chat) if chat.chat_type == ChatType::Private => "私聊".to_string(),
            Some(chat) => chat.title,
            None => String::new(),
        };
        let snapshot = match &quote {
            Some(quote) => quote.text.clone(),
            None => content.chars().take(SNAPSHOT_CHARS).collect(),
        };
        Ok((
            None,
            ReplyExtra {
                quote,
                cross: Some(CrossChatReply {
                    message_id,
                    chat_id: source_chat,
                    sender,
                    chat_title,
                    snapshot,
                }),
            },
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::locate_quote;

    #[test]
    fn quotes_must_be_real_slices_located_in_utf16() {
        let original = "你好 world, hello world";
        assert_eq!(
            locate_quote(original, "world", Some(16)),
            Some(16),
            "the requested match"
        );
        assert_eq!(
            locate_quote(original, "world", Some(1)),
            Some(3),
            "else the first match"
        );
        assert_eq!(locate_quote(original, "world", None), Some(3));
        assert_eq!(
            locate_quote("😀 smile", "smile", None),
            Some(3),
            "an emoji is two UTF-16 units"
        );
        assert_eq!(locate_quote(original, "planet", None), None);
        assert_eq!(locate_quote(original, "", None), None);
        assert_eq!(locate_quote("short", "much longer than it", None), None);
    }
}
