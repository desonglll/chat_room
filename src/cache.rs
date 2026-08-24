use std::time::Duration;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use redis::aio::ConnectionManager;
use uuid::Uuid;

use crate::config::RedisConfig;
use crate::models::User;

#[derive(Clone)]
pub(crate) struct SessionCache {
    manager: ConnectionManager,
    key_prefix: String,
    command_timeout: Duration,
}

impl SessionCache {
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
        })
    }

    pub async fn get(&self, token: Uuid) -> Result<Option<User>> {
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

    pub async fn set(&self, token: Uuid, user: &User, expires_at: DateTime<Utc>) -> Result<()> {
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

    pub async fn delete(&self, token: Uuid, user_id: Uuid) -> Result<()> {
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

    pub async fn delete_user(&self, user_id: Uuid) -> Result<()> {
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

    fn session_key(&self, token: Uuid) -> String {
        format!("{}:session:{token}", self.key_prefix)
    }

    fn user_sessions_key(&self, user_id: Uuid) -> String {
        format!("{}:user-sessions:{user_id}", self.key_prefix)
    }
}
