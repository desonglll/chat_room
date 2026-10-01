//! TG-408 link previews: the first http(s) link in a text message gets a card (Open Graph
//! title, description, image, site) fetched by the server **after** the message is stored and
//! delivered — a slow or failing site never delays or blocks a send. The card reaches the chat
//! in a `link_preview_updated` frame and rides on every later load. The sender can hide it
//! (or ask for none when sending). All fetching goes through `ssrf` + `fetch`.

pub mod fetch;
pub mod handlers;
pub mod parse;
pub mod ssrf;

use std::collections::HashMap;

use chrono::{DateTime, Duration, Utc};
use reqwest::Url;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, QueryBuilder};
use tokio::sync::Semaphore;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::models::{ChatMessage, StoredMessage};
use crate::state::{with_pool, AppState, SharedState};

/// Concurrent outbound fetches across the whole server.
static FETCHES: Semaphore = Semaphore::const_new(4);
const FRESH_OK: i64 = 24 * 60; // minutes
const FRESH_FAILED: i64 = 10;
const MAX_URL: usize = 2048;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct LinkPreview {
    pub url: String,
    pub site_name: String,
    pub title: String,
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_url: Option<String>,
}

/// The first http(s) link in `content`, without trailing punctuation.
pub fn first_url(content: &str) -> Option<Url> {
    let start = content
        .find("https://")
        .into_iter()
        .chain(content.find("http://"))
        .min()?;
    let token = content[start..]
        .split(|c: char| c.is_whitespace() || matches!(c, '<' | '>' | '"' | '`'))
        .next()?
        .trim_end_matches([
            '.', ',', ';', ':', '!', '?', ')', ']', '}', '\'', '。', '，', '）',
        ]);
    (token.len() <= MAX_URL)
        .then(|| Url::parse(token).ok())
        .flatten()
}

#[derive(FromRow)]
struct CachedRow {
    ok: bool,
    title: String,
    description: String,
    site_name: String,
    image_url: Option<String>,
    fetched_at: DateTime<Utc>,
}

impl AppState {
    pub(crate) fn link_preview_policy(&self) -> ssrf::Policy {
        let exempt = self.config.link_preview.allowed_sockets();
        let mut policy = ssrf::Policy::default();
        policy
            .ports
            .extend(exempt.iter().map(|socket| socket.port()));
        policy.exempt = exempt;
        policy
    }

    /// The cached preview for `url` if the cache entry is still fresh (`Some(None)` = a fresh
    /// failure), else `None`.
    async fn cached_preview(&self, url: &str) -> Result<Option<Option<LinkPreview>>, sqlx::Error> {
        let row: Option<CachedRow> = with_pool!(self, |pool| {
            sqlx::query_as(
                "SELECT ok, title, description, site_name, image_url, fetched_at \
                 FROM link_previews WHERE url = $1",
            )
            .bind(url)
            .fetch_optional(pool)
            .await
        })?;
        Ok(row.and_then(|row| {
            let fresh = Duration::minutes(if row.ok { FRESH_OK } else { FRESH_FAILED });
            (Utc::now() - row.fetched_at < fresh).then(|| {
                row.ok.then(|| LinkPreview {
                    url: url.to_string(),
                    site_name: row.site_name,
                    title: row.title,
                    description: row.description,
                    image_url: row.image_url,
                })
            })
        }))
    }

    async fn store_preview(
        &self,
        url: &str,
        preview: Option<&LinkPreview>,
    ) -> Result<(), sqlx::Error> {
        let empty = String::new();
        with_pool!(self, |pool| {
            sqlx::query(
                "INSERT INTO link_previews (url, ok, title, description, site_name, image_url, fetched_at) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7) \
                 ON CONFLICT (url) DO UPDATE SET ok = excluded.ok, title = excluded.title, \
                   description = excluded.description, site_name = excluded.site_name, \
                   image_url = excluded.image_url, fetched_at = excluded.fetched_at",
            )
            .bind(url)
            .bind(preview.is_some())
            .bind(preview.map_or(&empty, |p| &p.title))
            .bind(preview.map_or(&empty, |p| &p.description))
            .bind(preview.map_or(&empty, |p| &p.site_name))
            .bind(preview.and_then(|p| p.image_url.clone()))
            .bind(Utc::now())
            .execute(pool)
            .await
            .map(|_| ())
        })
    }

    /// A preview for `url`: from the cache, or fetched now under the SSRF policy and cached.
    pub(crate) async fn link_preview_for(
        &self,
        url: Url,
    ) -> Result<Option<LinkPreview>, sqlx::Error> {
        let key = url.to_string();
        if let Some(cached) = self.cached_preview(&key).await? {
            return Ok(cached);
        }
        let Ok(_permit) = FETCHES.acquire().await else {
            return Ok(None);
        };
        let fetched = fetch::fetch_preview(url, &self.link_preview_policy(), fetch::system_resolve)
            .await
            .unwrap_or_else(|error| {
                tracing::debug!("link preview fetch refused or failed: {error:?}");
                None
            });
        self.store_preview(&key, fetched.as_ref()).await?;
        Ok(fetched)
    }

    async fn build_message_preview(
        &self,
        room_id: Uuid,
        message_id: Uuid,
        url: Url,
    ) -> Result<Option<LinkPreview>, sqlx::Error> {
        with_pool!(self, |pool| {
            sqlx::query(
                "INSERT INTO message_link_previews (message_id, url) VALUES ($1, $2) \
                 ON CONFLICT (message_id) DO NOTHING",
            )
            .bind(message_id)
            .bind(url.as_str())
            .execute(pool)
            .await
            .map(|_| ())
        })?;
        let preview = self.link_preview_for(url).await?;
        let hidden: Option<bool> = with_pool!(self, |pool| {
            sqlx::query_scalar("SELECT hidden FROM message_link_previews WHERE message_id = $1")
                .bind(message_id)
                .fetch_optional(pool)
                .await
        })?;
        if hidden != Some(false) {
            return Ok(None);
        }
        if preview.is_some() {
            self.invalidate_message_cache(room_id).await;
        }
        Ok(preview)
    }

    /// Hide a message's card (its sender only). Works before the card exists too.
    pub(crate) async fn hide_link_preview(
        &self,
        room_id: Uuid,
        sender_id: Uuid,
        message_id: Uuid,
    ) -> Result<bool, sqlx::Error> {
        let changed = with_pool!(self, |pool| {
            sqlx::query(
                "INSERT INTO message_link_previews (message_id, url, hidden) \
                 SELECT id, '', TRUE FROM messages \
                 WHERE id = $1 AND room_id = $2 AND sender_id = $3 AND recalled_at IS NULL \
                 ON CONFLICT (message_id) DO UPDATE SET hidden = TRUE",
            )
            .bind(message_id)
            .bind(room_id)
            .bind(sender_id)
            .execute(pool)
            .await
            .map(|result| result.rows_affected())
        })?;
        if changed > 0 {
            self.invalidate_message_cache(room_id).await;
        }
        Ok(changed > 0)
    }

    /// Attach cards to loaded messages (every loader runs this, like TG-410 contacts).
    pub(crate) async fn attach_link_previews(
        &self,
        messages: &mut [StoredMessage],
    ) -> Result<(), sqlx::Error> {
        let ids: Vec<Uuid> = messages
            .iter()
            .filter(|message| message.recalled_at.is_none() && message.content.contains("http"))
            .map(|message| message.id)
            .collect();
        if ids.is_empty() {
            return Ok(());
        }
        let rows: Vec<(Uuid, String, String, String, String, Option<String>)> =
            with_pool!(self, |pool| {
                let mut query = QueryBuilder::new(
                    "SELECT links.message_id, previews.url, previews.site_name, previews.title, \
                   previews.description, previews.image_url \
                 FROM message_link_previews AS links \
                 JOIN link_previews AS previews ON previews.url = links.url AND previews.ok \
                 WHERE NOT links.hidden AND links.message_id IN (",
                );
                {
                    let mut values = query.separated(", ");
                    for id in &ids {
                        values.push_bind(*id);
                    }
                }
                query.push(")");
                query.build_query_as().fetch_all(pool).await
            })?;
        let mut cards: HashMap<Uuid, LinkPreview> = rows
            .into_iter()
            .map(|(id, url, site_name, title, description, image_url)| {
                (
                    id,
                    LinkPreview {
                        url,
                        site_name,
                        title,
                        description,
                        image_url,
                    },
                )
            })
            .collect();
        for message in messages.iter_mut() {
            message.link_preview = cards.remove(&message.id);
        }
        Ok(())
    }
}

/// Called after a text message is stored (`wanted`: newly inserted and the sender did not
/// dismiss the card): builds its card in the background, then tells the chat. The send path
/// never waits for it — a slow or failing site cannot delay or block a message.
pub(crate) fn build_link_card(
    state: &SharedState,
    room_id: Uuid,
    message: &StoredMessage,
    wanted: bool,
) {
    if !wanted || !state.config.link_preview.enabled {
        return;
    }
    let Some(url) = first_url(&message.content) else {
        return;
    };
    let message_id = message.id;
    let state = state.clone();
    tokio::spawn(async move {
        match state.build_message_preview(room_id, message_id, url).await {
            Ok(Some(preview)) => {
                state
                    .broadcast(
                        room_id,
                        ChatMessage::LinkPreviewUpdated {
                            message_id,
                            preview: Some(preview),
                        },
                    )
                    .await
            }
            Ok(None) => {}
            Err(error) => tracing::warn!(%room_id, "link preview failed: {error}"),
        }
    });
}

#[cfg(test)]
mod tests {
    use super::first_url;

    #[test]
    fn the_first_link_is_taken_without_trailing_punctuation() {
        let url = |text: &str| first_url(text).map(|url| url.to_string());
        assert_eq!(
            url("see https://example.com/a)."),
            Some("https://example.com/a".into())
        );
        assert_eq!(
            url("a http://x.org, then https://y.org"),
            Some("http://x.org/".into())
        );
        assert_eq!(
            url("看这个：https://例子.测试/路径。"),
            Some("https://xn--fsqu00a.xn--0zwm56d/%E8%B7%AF%E5%BE%84".into())
        );
        assert_eq!(url("no links here"), None);
        assert_eq!(url("ftp://example.com"), None);
    }
}
