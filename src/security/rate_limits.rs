use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

use redis::Script;
use serde::Serialize;
use sha2::{Digest, Sha256};
use tokio::sync::Mutex;
use utoipa::ToSchema;

use crate::{cache::RedisCache, config::AuthConfig};

const LIMIT_SCRIPT: &str = r#"
local ip_count = redis.call('INCR', KEYS[1])
if ip_count == 1 then redis.call('EXPIRE', KEYS[1], ARGV[1]) end
local account_count = redis.call('INCR', KEYS[2])
if account_count == 1 then redis.call('EXPIRE', KEYS[2], ARGV[1]) end
if ip_count > tonumber(ARGV[2]) or account_count > tonumber(ARGV[3]) then
  return 0
end
return 1
"#;

#[derive(Clone, Copy)]
pub(crate) enum AuthAction {
    Login,
    Register,
    VerifyPassword,
}

impl AuthAction {
    fn key(self) -> &'static str {
        match self {
            Self::Login => "login",
            Self::Register => "register",
            Self::VerifyPassword => "verify-password",
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct AuthRateLimitSnapshot {
    pub adapter: &'static str,
    pub allowed: u64,
    pub blocked: u64,
    pub redis_fallbacks: u64,
}

#[derive(Clone)]
pub(crate) struct AuthRateLimits {
    inner: Arc<Inner>,
}

struct Inner {
    window: Duration,
    ip_limit: u64,
    account_limit: u64,
    local: Mutex<HashMap<String, LocalWindow>>,
    redis: Option<RedisCache>,
    allowed: AtomicU64,
    blocked: AtomicU64,
    redis_fallbacks: AtomicU64,
}

struct LocalWindow {
    started_at: Instant,
    attempts: u64,
}

impl AuthRateLimits {
    pub(crate) fn new(config: &AuthConfig, redis: Option<RedisCache>) -> Self {
        Self {
            inner: Arc::new(Inner {
                window: Duration::from_secs(config.rate_limit_window_secs),
                ip_limit: config.rate_limit_ip_attempts,
                account_limit: config.rate_limit_account_attempts,
                local: Mutex::new(HashMap::new()),
                redis,
                allowed: AtomicU64::new(0),
                blocked: AtomicU64::new(0),
                redis_fallbacks: AtomicU64::new(0),
            }),
        }
    }

    pub(crate) async fn check(
        &self,
        action: AuthAction,
        client_address: &str,
        account: &str,
    ) -> bool {
        let ip_key = digest_key(action, "ip", client_address);
        let account_key = digest_key(action, "account", &account.to_lowercase());
        let allowed = match &self.inner.redis {
            Some(redis) => match self.check_redis(redis, &ip_key, &account_key).await {
                Ok(allowed) => allowed,
                Err(()) => {
                    let fallbacks = self.inner.redis_fallbacks.fetch_add(1, Ordering::Relaxed);
                    if fallbacks == 0 {
                        tracing::warn!("Redis auth limiter unavailable; using local fallback");
                    }
                    self.check_local(ip_key, account_key).await
                }
            },
            None => self.check_local(ip_key, account_key).await,
        };
        let metric = if allowed {
            &self.inner.allowed
        } else {
            &self.inner.blocked
        };
        metric.fetch_add(1, Ordering::Relaxed);
        allowed
    }

    pub(crate) fn snapshot(&self) -> AuthRateLimitSnapshot {
        AuthRateLimitSnapshot {
            adapter: if self.inner.redis.is_some() {
                "redis"
            } else {
                "local"
            },
            allowed: self.inner.allowed.load(Ordering::Relaxed),
            blocked: self.inner.blocked.load(Ordering::Relaxed),
            redis_fallbacks: self.inner.redis_fallbacks.load(Ordering::Relaxed),
        }
    }

    async fn check_redis(
        &self,
        redis: &RedisCache,
        ip_key: &str,
        account_key: &str,
    ) -> Result<bool, ()> {
        let mut connection = redis.manager.clone();
        let prefix = redis.key_prefix.trim_end_matches(':');
        let result = tokio::time::timeout(
            redis.command_timeout,
            Script::new(LIMIT_SCRIPT)
                .key(format!("{prefix}:auth-rate:{ip_key}"))
                .key(format!("{prefix}:auth-rate:{account_key}"))
                .arg(self.inner.window.as_secs())
                .arg(self.inner.ip_limit)
                .arg(self.inner.account_limit)
                .invoke_async::<i64>(&mut connection),
        )
        .await
        .map_err(|_| ())?
        .map_err(|_| ())?;
        Ok(result == 1)
    }

    async fn check_local(&self, ip_key: String, account_key: String) -> bool {
        let now = Instant::now();
        let mut limits = self.inner.local.lock().await;
        limits.retain(|_, entry| now.duration_since(entry.started_at) < self.inner.window);
        let ip_count = increment(&mut limits, ip_key, now, self.inner.window);
        let account_count = increment(&mut limits, account_key, now, self.inner.window);
        ip_count <= self.inner.ip_limit && account_count <= self.inner.account_limit
    }
}

fn increment(
    limits: &mut HashMap<String, LocalWindow>,
    key: String,
    now: Instant,
    window: Duration,
) -> u64 {
    let entry = limits.entry(key).or_insert(LocalWindow {
        started_at: now,
        attempts: 0,
    });
    if now.duration_since(entry.started_at) >= window {
        entry.started_at = now;
        entry.attempts = 0;
    }
    entry.attempts += 1;
    entry.attempts
}

fn digest_key(action: AuthAction, dimension: &str, value: &str) -> String {
    let digest = Sha256::digest(format!("{}:{dimension}:{value}", action.key()).as_bytes());
    format!("{}:{dimension}:{}", action.key(), hex::encode(digest))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn local_adapter_enforces_both_dimensions() {
        let limits = AuthRateLimits::new(
            &AuthConfig {
                rate_limit_window_secs: 60,
                rate_limit_ip_attempts: 3,
                rate_limit_account_attempts: 2,
                ..AuthConfig::default()
            },
            None,
        );
        assert!(limits.check(AuthAction::Login, "ip-a", "account-a").await);
        assert!(limits.check(AuthAction::Login, "ip-a", "account-a").await);
        assert!(!limits.check(AuthAction::Login, "ip-a", "account-a").await);
        assert!(limits.check(AuthAction::Login, "ip-b", "account-b").await);
        assert!(limits.check(AuthAction::Login, "ip-b", "account-c").await);
        assert!(limits.check(AuthAction::Login, "ip-b", "account-d").await);
        assert!(!limits.check(AuthAction::Login, "ip-b", "account-e").await);
        let snapshot = limits.snapshot();
        assert_eq!(snapshot.allowed, 5);
        assert_eq!(snapshot.blocked, 2);
        assert_eq!(snapshot.redis_fallbacks, 0);
    }
}
