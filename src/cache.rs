use std::time::Duration;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use redis::aio::ConnectionManager;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::config::RedisConfig;
use crate::message_store::MessageCursor;
use crate::models::{StoredMessage, User};

#[derive(Clone)]
pub(crate) struct RedisCache {
    manager: ConnectionManager,
    key_prefix: String,
    command_timeout: Duration,
    message_ttl_secs: u64,
}

pub(crate) enum MessageCacheLookup {
    Hit(Vec<StoredMessage>),
    Miss(MessageCacheTicket),
}

pub(crate) struct MessageCacheTicket(String);

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct CachedAiAnswer {
    pub content: String,
    pub context_message_count: i64,
    pub revision: i64,
    pub updated_at: DateTime<Utc>,
}

impl RedisCache {
    pub async fn connect(config: &RedisConfig) -> Result<Self> {
        let client = redis::Client::open(config.url.as_str()).context("parse Redis URL")?;
        let mut manager = tokio::time::timeout(
            Duration::from_millis(config.connect_timeout_ms),
            client.get_connection_manager(),
        )
        .await
        .context("connect to Redis timed out")?
        .context("connect to Redis")?;
        tokio::time::timeout(
            Duration::from_millis(config.command_timeout_ms),
            redis::cmd("PING").query_async::<String>(&mut manager),
        )
        .await
        .context("Redis PING timed out")?
        .context("Redis PING failed")?;
        Ok(Self {
            manager,
            key_prefix: config.key_prefix.trim_end_matches(':').to_string(),
            command_timeout: Duration::from_millis(config.command_timeout_ms),
            message_ttl_secs: config.message_ttl_secs,
        })
    }

    pub async fn get_session(&self, token: Uuid) -> Result<Option<User>> {
        let mut connection = self.manager.clone();
        let value = tokio::time::timeout(
            self.command_timeout,
            redis::cmd("GET")
                .arg(self.session_key(token))
                .query_async::<Option<String>>(&mut connection),
        )
        .await
        .context("Redis GET timed out")?
        .context("read cached session")?;
        value
            .map(|json| serde_json::from_str(&json).context("decode cached session"))
            .transpose()
    }

    pub async fn set_session(
        &self,
        token: Uuid,
        user: &User,
        expires_at: DateTime<Utc>,
    ) -> Result<()> {
        let ttl_seconds = (expires_at - Utc::now()).num_seconds().max(1);
        let session_key = self.session_key(token);
        let user_key = self.user_sessions_key(user.id);
        let json = serde_json::to_string(user).context("encode cached session")?;
        let mut connection = self.manager.clone();
        tokio::time::timeout(
            self.command_timeout,
            redis::pipe()
                .atomic()
                .cmd("SET")
                .arg(&session_key)
                .arg(json)
                .arg("EX")
                .arg(ttl_seconds)
                .ignore()
                .cmd("SADD")
                .arg(&user_key)
                .arg(&session_key)
                .ignore()
                .cmd("EXPIRE")
                .arg(&user_key)
                .arg(ttl_seconds)
                .ignore()
                .query_async::<()>(&mut connection),
        )
        .await
        .context("Redis session write timed out")?
        .context("write cached session")
    }

    pub async fn delete_session(&self, token: Uuid, user_id: Uuid) -> Result<()> {
        let session_key = self.session_key(token);
        let user_key = self.user_sessions_key(user_id);
        let mut connection = self.manager.clone();
        tokio::time::timeout(
            self.command_timeout,
            redis::pipe()
                .atomic()
                .cmd("DEL")
                .arg(&session_key)
                .ignore()
                .cmd("SREM")
                .arg(&user_key)
                .arg(&session_key)
                .ignore()
                .query_async::<()>(&mut connection),
        )
        .await
        .context("Redis session delete timed out")?
        .context("delete cached session")
    }

    pub async fn delete_user_sessions(&self, user_id: Uuid) -> Result<()> {
        let user_key = self.user_sessions_key(user_id);
        let mut connection = self.manager.clone();
        let session_keys = tokio::time::timeout(
            self.command_timeout,
            redis::cmd("SMEMBERS")
                .arg(&user_key)
                .query_async::<Vec<String>>(&mut connection),
        )
        .await
        .context("Redis SMEMBERS timed out")?
        .context("list cached user sessions")?;
        let mut keys = session_keys;
        keys.push(user_key);
        tokio::time::timeout(
            self.command_timeout,
            redis::cmd("DEL")
                .arg(keys)
                .query_async::<usize>(&mut connection),
        )
        .await
        .context("Redis user session delete timed out")?
        .context("delete cached user sessions")?;
        Ok(())
    }

    pub async fn clear_all(&self) -> Result<usize> {
        let mut connection = self.manager.clone();
        let mut cursor = 0_u64;
        let mut deleted = 0;
        loop {
            let (next, keys) = tokio::time::timeout(
                self.command_timeout,
                redis::cmd("SCAN")
                    .arg(cursor)
                    .arg("MATCH")
                    .arg(format!("{}:*", self.key_prefix))
                    .arg("COUNT")
                    .arg(500)
                    .query_async::<(u64, Vec<String>)>(&mut connection),
            )
            .await
            .context("Redis SCAN timed out")?
            .context("scan cached sessions")?;
            if !keys.is_empty() {
                deleted += tokio::time::timeout(
                    self.command_timeout,
                    redis::cmd("DEL")
                        .arg(keys)
                        .query_async::<usize>(&mut connection),
                )
                .await
                .context("Redis cache clear timed out")?
                .context("clear cached sessions")?;
            }
            cursor = next;
            if cursor == 0 {
                return Ok(deleted);
            }
        }
    }

    pub async fn message_history(
        &self,
        room_id: Uuid,
        limit: i64,
        through: Option<&MessageCursor>,
        viewer_id: Option<Uuid>,
    ) -> Result<MessageCacheLookup> {
        let version = self.message_version(room_id).await?;
        let key = self.message_history_key(room_id, version, limit, through, viewer_id);
        let mut connection = self.manager.clone();
        let value = tokio::time::timeout(
            self.command_timeout,
            redis::cmd("GET")
                .arg(&key)
                .query_async::<Option<String>>(&mut connection),
        )
        .await
        .context("Redis message GET timed out")?
        .context("read cached message history")?;
        match value {
            Some(json) => Ok(MessageCacheLookup::Hit(
                serde_json::from_str(&json).context("decode cached message history")?,
            )),
            None => Ok(MessageCacheLookup::Miss(MessageCacheTicket(key))),
        }
    }

    pub async fn set_message_history(
        &self,
        ticket: MessageCacheTicket,
        messages: &[StoredMessage],
    ) -> Result<()> {
        let json = serde_json::to_string(messages).context("encode cached message history")?;
        let mut connection = self.manager.clone();
        tokio::time::timeout(
            self.command_timeout,
            redis::cmd("SET")
                .arg(ticket.0)
                .arg(json)
                .arg("EX")
                .arg(self.message_ttl_secs)
                .query_async::<()>(&mut connection),
        )
        .await
        .context("Redis message SET timed out")?
        .context("cache message history")
    }

    pub async fn invalidate_message_history(&self, room_id: Uuid) -> Result<()> {
        let mut connection = self.manager.clone();
        tokio::time::timeout(
            self.command_timeout,
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
        .context("Redis message invalidation timed out")?
        .context("invalidate cached message history")
    }

    pub async fn ai_answer(&self, message_id: Uuid) -> Result<Option<CachedAiAnswer>> {
        let mut connection = self.manager.clone();
        let value = tokio::time::timeout(
            self.command_timeout,
            redis::cmd("GET")
                .arg(self.ai_answer_key(message_id))
                .query_async::<Option<String>>(&mut connection),
        )
        .await
        .context("Redis AI answer GET timed out")?
        .context("read cached AI answer")?;
        value
            .map(|json| serde_json::from_str(&json).context("decode cached AI answer"))
            .transpose()
    }

    pub async fn set_ai_answer(
        &self,
        message_id: Uuid,
        answer: &CachedAiAnswer,
        ttl_secs: u64,
    ) -> Result<()> {
        let json = serde_json::to_string(answer).context("encode cached AI answer")?;
        let mut connection = self.manager.clone();
        tokio::time::timeout(
            self.command_timeout,
            redis::cmd("SET")
                .arg(self.ai_answer_key(message_id))
                .arg(json)
                .arg("EX")
                .arg(ttl_secs.max(60))
                .query_async::<()>(&mut connection),
        )
        .await
        .context("Redis AI answer SET timed out")?
        .context("cache AI answer")
    }

    pub async fn delete_ai_answer(&self, message_id: Uuid) -> Result<()> {
        let mut connection = self.manager.clone();
        tokio::time::timeout(
            self.command_timeout,
            redis::cmd("DEL")
                .arg(self.ai_answer_key(message_id))
                .query_async::<usize>(&mut connection),
        )
        .await
        .context("Redis AI answer DEL timed out")?
        .context("delete cached AI answer")?;
        Ok(())
    }

    async fn message_version(&self, room_id: Uuid) -> Result<u64> {
        let mut connection = self.manager.clone();
        tokio::time::timeout(
            self.command_timeout,
            redis::cmd("GET")
                .arg(self.message_version_key(room_id))
                .query_async::<Option<u64>>(&mut connection),
        )
        .await
        .context("Redis message version GET timed out")?
        .context("read message cache version")
        .map(|version| version.unwrap_or(0))
    }

    fn session_key(&self, token: Uuid) -> String {
        format!("{}:session:{token}", self.key_prefix)
    }

    fn user_sessions_key(&self, user_id: Uuid) -> String {
        format!("{}:user-sessions:{user_id}", self.key_prefix)
    }

    fn message_version_key(&self, room_id: Uuid) -> String {
        format!("{}:messages:{room_id}:version", self.key_prefix)
    }

    fn ai_answer_key(&self, message_id: Uuid) -> String {
        format!("{}:ai-answer:{message_id}", self.key_prefix)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn live_ai_answers_expire_from_redis_after_persistence() {
        let config = RedisConfig {
            enabled: true,
            key_prefix: format!("chat-room-test:{}", Uuid::new_v4()),
            ..RedisConfig::default()
        };
        let Ok(cache) = RedisCache::connect(&config).await else {
            eprintln!("skipping Redis AI answer lifecycle test: Redis is unavailable");
            return;
        };
        let message_id = Uuid::new_v4();
        let answer = CachedAiAnswer {
            content: "partial answer".into(),
            context_message_count: 3,
            revision: 2,
            updated_at: Utc::now(),
        };

        cache.set_ai_answer(message_id, &answer, 60).await.unwrap();
        let cached = cache.ai_answer(message_id).await.unwrap().unwrap();
        assert_eq!(cached.content, "partial answer");
        assert_eq!(cached.revision, 2);

        cache.delete_ai_answer(message_id).await.unwrap();
        assert!(cache.ai_answer(message_id).await.unwrap().is_none());
    }
}
