mod breaker;
mod messages;

use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Utc};
use redis::aio::ConnectionManager;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::ai_threads::{AiCitationSource, AiRunTraceStep};
use crate::config::RedisConfig;
use crate::models::{StoredMessage, User};
use breaker::{Admission, Breaker};

#[derive(Clone)]
pub(crate) struct RedisCache {
    pub(crate) manager: ConnectionManager,
    pub(crate) key_prefix: String,
    pub(crate) command_timeout: Duration,
    message_ttl_secs: u64,
    breaker: Breaker,
}

pub(crate) enum MessageCacheLookup {
    Hit(Vec<StoredMessage>),
    Miss(MessageCacheTicket),
    /// Redis is bypassed after a recent failure; read the database and do not populate.
    Bypass,
}

pub(crate) struct MessageCacheTicket(String);

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct CachedAiAnswer {
    pub content: String,
    pub context_message_count: i64,
    #[serde(default)]
    pub retrieved_message_count: i64,
    #[serde(default)]
    pub sources: Vec<AiCitationSource>,
    #[serde(default)]
    pub trace: Vec<AiRunTraceStep>,
    pub revision: i64,
    #[serde(default = "streaming_status")]
    pub status: String,
    #[serde(default = "responding_stage")]
    pub stage: String,
    #[serde(default)]
    pub stage_started_at: Option<DateTime<Utc>>,
    pub updated_at: DateTime<Utc>,
}

fn streaming_status() -> String {
    "streaming".into()
}

fn responding_stage() -> String {
    "responding".into()
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
            breaker: Breaker::new(breaker::OPEN_COOLDOWN),
        })
    }

    /// Runs one Redis command behind the circuit breaker and the command timeout.
    /// `Ok(None)` means the breaker is open and Redis was skipped: fall back to the database.
    pub(crate) async fn run<T>(
        &self,
        command: impl std::future::Future<Output = redis::RedisResult<T>>,
    ) -> Result<Option<T>> {
        if self.breaker.admit(Instant::now()) == Admission::Bypass {
            return Ok(None);
        }
        let outcome = tokio::time::timeout(self.command_timeout, command).await;
        let transport_ok = match &outcome {
            Ok(Ok(_)) => true,
            // A server reply (e.g. WRONGTYPE) proves Redis is reachable.
            Ok(Err(error)) => !is_transport_failure(error),
            Err(_) => false,
        };
        if self.breaker.record(Instant::now(), transport_ok) {
            if transport_ok {
                tracing::info!("Redis reachable again; cache re-enabled");
            } else {
                tracing::warn!(
                    "Redis slow or unreachable; using database reads for {}s",
                    breaker::OPEN_COOLDOWN.as_secs()
                );
            }
        }
        match outcome {
            Ok(Ok(value)) => Ok(Some(value)),
            Ok(Err(error)) => Err(error).context("Redis command failed"),
            Err(_) => Err(anyhow!(
                "Redis command timed out after {} ms",
                self.command_timeout.as_millis()
            )),
        }
    }

    pub(crate) async fn ping(&self) -> Result<()> {
        let mut connection = self.manager.clone();
        self.run(redis::cmd("PING").query_async::<String>(&mut connection))
            .await
            .context("Redis PING failed")?
            .ok_or_else(|| anyhow!("Redis bypassed after a recent failure"))?;
        Ok(())
    }

    pub async fn get_session(&self, token: Uuid) -> Result<Option<User>> {
        let mut connection = self.manager.clone();
        let value = self
            .run(
                redis::cmd("GET")
                    .arg(self.session_key(token))
                    .query_async::<Option<String>>(&mut connection),
            )
            .await
            .context("read cached session")?
            .flatten();
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
        self.run(
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
        .context("write cached session")?;
        Ok(())
    }

    // Skipping a session delete while Redis is bypassed is safe: `session_user` checks the
    // database row on every request, so a revoked session never authenticates from cache.
    pub async fn delete_session(&self, token: Uuid, user_id: Uuid) -> Result<()> {
        let session_key = self.session_key(token);
        let user_key = self.user_sessions_key(user_id);
        let mut connection = self.manager.clone();
        self.run(
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
        .context("delete cached session")?;
        Ok(())
    }

    pub async fn delete_user_sessions(&self, user_id: Uuid) -> Result<()> {
        let user_key = self.user_sessions_key(user_id);
        let mut connection = self.manager.clone();
        let Some(mut keys) = self
            .run(
                redis::cmd("SMEMBERS")
                    .arg(&user_key)
                    .query_async::<Vec<String>>(&mut connection),
            )
            .await
            .context("list cached user sessions")?
        else {
            return Ok(());
        };
        keys.push(user_key);
        self.run(
            redis::cmd("DEL")
                .arg(keys)
                .query_async::<usize>(&mut connection),
        )
        .await
        .context("delete cached user sessions")?;
        Ok(())
    }

    pub async fn clear_all(&self) -> Result<usize> {
        let mut connection = self.manager.clone();
        let mut cursor = 0_u64;
        let mut deleted = 0;
        loop {
            let (next, keys) = self
                .run(
                    redis::cmd("SCAN")
                        .arg(cursor)
                        .arg("MATCH")
                        .arg(format!("{}:*", self.key_prefix))
                        .arg("COUNT")
                        .arg(500)
                        .query_async::<(u64, Vec<String>)>(&mut connection),
                )
                .await
                .context("scan cached sessions")?
                .ok_or_else(|| anyhow!("Redis bypassed after a recent failure"))?;
            if !keys.is_empty() {
                deleted += self
                    .run(
                        redis::cmd("DEL")
                            .arg(keys)
                            .query_async::<usize>(&mut connection),
                    )
                    .await
                    .context("clear cached sessions")?
                    .ok_or_else(|| anyhow!("Redis bypassed after a recent failure"))?;
            }
            cursor = next;
            if cursor == 0 {
                return Ok(deleted);
            }
        }
    }

    pub async fn ai_answer(&self, message_id: Uuid) -> Result<Option<CachedAiAnswer>> {
        let mut connection = self.manager.clone();
        let value = self
            .run(
                redis::cmd("GET")
                    .arg(self.ai_answer_key(message_id))
                    .query_async::<Option<String>>(&mut connection),
            )
            .await
            .context("read cached AI answer")?
            .flatten();
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
        self.run(
            redis::cmd("SET")
                .arg(self.ai_answer_key(message_id))
                .arg(json)
                .arg("EX")
                .arg(ttl_secs.max(60))
                .query_async::<()>(&mut connection),
        )
        .await
        .context("cache AI answer")?;
        Ok(())
    }

    fn session_key(&self, token: Uuid) -> String {
        format!("{}:session:{token}", self.key_prefix)
    }

    fn user_sessions_key(&self, user_id: Uuid) -> String {
        format!("{}:user-sessions:{user_id}", self.key_prefix)
    }

    fn ai_answer_key(&self, message_id: Uuid) -> String {
        format!("{}:ai-answer:{message_id}", self.key_prefix)
    }
}

#[cfg(test)]
mod stall_tests;

fn is_transport_failure(error: &redis::RedisError) -> bool {
    error.is_io_error()
        || error.is_timeout()
        || error.is_connection_dropped()
        || error.is_connection_refusal()
        || error.is_unrecoverable_error()
}

#[cfg(test)]
#[path = "cache_tests.rs"]
mod tests;
