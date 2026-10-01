//! Message-history page cache, split out of `src/cache.rs` by responsibility (TG-1207).

use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use uuid::Uuid;

use super::{MessageCacheLookup, MessageCacheTicket, RedisCache};
use crate::message_store::MessageCursor;
use crate::models::StoredMessage;

impl RedisCache {
    pub async fn message_history(
        &self,
        room_id: Uuid,
        limit: i64,
        through: Option<&MessageCursor>,
        viewer_id: Option<Uuid>,
    ) -> Result<MessageCacheLookup> {
        let Some(version) = self.message_version(room_id).await? else {
            return Ok(MessageCacheLookup::Bypass);
        };
        let key = self.message_history_key(room_id, version, limit, through, viewer_id);
        let ttl = Duration::from_secs(self.message_ttl_secs);
        if !self.breaker.trusts_entries_older_than(Instant::now(), ttl) {
            // A recent failure may have lost an invalidation: read the database and overwrite.
            return Ok(MessageCacheLookup::Miss(MessageCacheTicket(key)));
        }
        let mut connection = self.manager.clone();
        let value = self
            .run(
                redis::cmd("GET")
                    .arg(&key)
                    .query_async::<Option<String>>(&mut connection),
            )
            .await
            .context("read cached message history")?;
        match value {
            None => Ok(MessageCacheLookup::Bypass),
            Some(Some(json)) => Ok(MessageCacheLookup::Hit(
                serde_json::from_str(&json).context("decode cached message history")?,
            )),
            Some(None) => Ok(MessageCacheLookup::Miss(MessageCacheTicket(key))),
        }
    }

    pub async fn set_message_history(
        &self,
        ticket: MessageCacheTicket,
        messages: &[StoredMessage],
    ) -> Result<()> {
        let json = serde_json::to_string(messages).context("encode cached message history")?;
        let mut connection = self.manager.clone();
        self.run(
            redis::cmd("SET")
                .arg(ticket.0)
                .arg(json)
                .arg("EX")
                .arg(self.message_ttl_secs)
                .query_async::<()>(&mut connection),
        )
        .await
        .context("cache message history")?;
        Ok(())
    }

    // A skipped or failed invalidation is covered by `Breaker::trusts_entries_older_than`.
    pub async fn invalidate_message_history(&self, room_id: Uuid) -> Result<()> {
        let mut connection = self.manager.clone();
        self.run(
            redis::pipe()
                .atomic()
                .cmd("INCR")
                .arg(self.message_version_key(room_id))
                .ignore()
                .cmd("EXPIRE")
                .arg(self.message_version_key(room_id))
                .arg(self.message_ttl_secs.max(900))
                .ignore()
                .query_async::<()>(&mut connection),
        )
        .await
        .context("invalidate cached message history")?;
        Ok(())
    }

    async fn message_version(&self, room_id: Uuid) -> Result<Option<u64>> {
        let mut connection = self.manager.clone();
        Ok(self
            .run(
                redis::cmd("GET")
                    .arg(self.message_version_key(room_id))
                    .query_async::<Option<u64>>(&mut connection),
            )
            .await
            .context("read message cache version")?
            .map(|version| version.unwrap_or(0)))
    }

    fn message_version_key(&self, room_id: Uuid) -> String {
        format!("{}:messages:{room_id}:version", self.key_prefix)
    }

    fn message_history_key(
        &self,
        room_id: Uuid,
        version: u64,
        limit: i64,
        through: Option<&MessageCursor>,
        viewer_id: Option<Uuid>,
    ) -> String {
        let through = through
            .map(|cursor| format!("{}-{}", cursor.created_at.timestamp_micros(), cursor.id))
            .unwrap_or_else(|| "latest".into());
        let viewer = viewer_id
            .map(|id| id.to_string())
            .unwrap_or_else(|| "public".into());
        format!(
            "{}:messages:{room_id}:v{version}:{viewer}:{limit}:{through}",
            self.key_prefix
        )
    }
}
