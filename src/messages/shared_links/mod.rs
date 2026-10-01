//! TG-803: the chat info panel's «链接» tab — every http(s) link shared in a chat, newest first.
//!
//! `message_links` is a projection of message text, refreshed lazily per chat when the tab is
//! read ([`AppState::refresh_link_index`]): messages after the chat's index cursor are
//! extracted (which is also how existing history is backfilled — on first read, in bounded
//! batches), and messages edited since the last refresh are re-extracted. Recalled messages
//! are filtered at read time, together with the same membership gate as `/files`.
//! Links come from the text and from `text_link` entities (a link hidden behind words).

pub mod handlers;
mod store;

use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

use chrono::{DateTime, Utc};

const MAX_URL: usize = 2048;
/// Links kept per message; a message pasting a hundred URLs should not flood the tab.
const MAX_LINKS_PER_MESSAGE: usize = 20;

/// One row of the «链接» tab.
#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct SharedLinkItem {
    pub message_id: Uuid,
    pub position: i32,
    pub url: String,
    pub sender_id: Option<Uuid>,
    pub sender: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct SharedLinkPage {
    pub items: Vec<SharedLinkItem>,
    /// Opaque cursor for the next (older) page; absent on the last page.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next: Option<String>,
}

/// Every http(s) link in `content` (then the `text_link` targets), in order, deduplicated,
/// without trailing punctuation.
pub fn extract_urls<'a>(
    content: &str,
    text_links: impl IntoIterator<Item = &'a str>,
) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    let mut push = |candidate: &str| {
        if candidate.len() > MAX_URL || found.len() >= MAX_LINKS_PER_MESSAGE {
            return;
        }
        let Ok(url) = reqwest::Url::parse(candidate) else {
            return;
        };
        if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
            return;
        }
        let url = url.to_string();
        if !found.contains(&url) {
            found.push(url);
        }
    };
    let mut rest = content;
    while let Some(start) = next_scheme(rest) {
        let tail = &rest[start..];
        let token = tail
            .split(|c: char| {
                c.is_whitespace()
                    || matches!(c, '<' | '>' | '"' | '`')
                    // Chinese text rarely puts a space after a link; full-width punctuation
                    // ends it (stricter than TG-408's `first_url`, which only trims).
                    || matches!(c, '。' | '，' | '、' | '；' | '：' | '！' | '？' | '）' | '」' | '』')
            })
            .next()
            .unwrap_or_default();
        push(token.trim_end_matches([
            '.', ',', ';', ':', '!', '?', ')', ']', '}', '\'', '。', '，', '）',
        ]));
        rest = &tail[token.len().max(1)..];
    }
    for link in text_links {
        push(link);
    }
    found
}

fn next_scheme(text: &str) -> Option<usize> {
    let lower = text.to_ascii_lowercase();
    [lower.find("https://"), lower.find("http://")]
        .into_iter()
        .flatten()
        .min()
}

#[cfg(test)]
mod tests {
    use super::extract_urls;

    #[test]
    fn finds_every_link_in_order_without_trailing_punctuation() {
        let urls = extract_urls(
            "see https://a.example/x, and (http://b.example/y). again https://a.example/x",
            [],
        );
        assert_eq!(urls, ["https://a.example/x", "http://b.example/y"]);
    }

    #[test]
    fn text_links_follow_and_junk_is_ignored() {
        let urls = extract_urls(
            "no links: http:// ftp://x.example",
            ["https://hidden.example/"],
        );
        assert_eq!(urls, ["https://hidden.example/"]);
    }

    #[test]
    fn chinese_punctuation_and_mixed_case_schemes() {
        let urls = extract_urls("链接：HTTPS://c.example/路径。好的", []);
        assert_eq!(urls, ["https://c.example/%E8%B7%AF%E5%BE%84"]);
    }
}
