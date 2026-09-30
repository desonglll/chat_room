//! Visible skipping for tests that need an external service (PostgreSQL, Redis).
//!
//! The contract, shared by every service-backed test in this suite:
//!
//! * Env var **unset** → the test skips, but never silently: it prints one standardized
//!   marker line to the *real* stderr. Plain `eprintln!` will not do — libtest captures a
//!   passing test's output and discards it, which is exactly how the old "skipping …"
//!   messages stayed invisible. Writing to the stderr file descriptor bypasses capture,
//!   so a normal `cargo test` run shows at a glance which coverage was not verified.
//! * Env var **set** → the service is required: an unreachable service panics the test
//!   instead of skipping, so a broken environment cannot hide behind a green run.
//!
//! There is deliberately no probe-and-fall-back URL: a fallback that cannot connect
//! disguises "misconfigured" as "not configured" (the TG-013 defect — the old fallback's
//! credentials never matched `docker-compose.local.yaml`, so every PostgreSQL test
//! silently skipped on developer machines).
//!
//! Count a full run's unverified coverage with:
//!
//! ```sh
//! cargo test --all-targets --all-features 2>&1 | grep -c 'SKIPPED: PostgreSQL not verified'
//! ```
//!
//! Local services come from `docker-compose.local.yaml`; its comments name the env var
//! each service maps to.

#![allow(dead_code)]

use std::io::Write;

/// Admin-capable PostgreSQL URL (needs `CREATE DATABASE` rights).
pub const POSTGRES_ENV: &str = "TEST_POSTGRES_ADMIN_URL";
/// Redis URL for tests, with the runtime variable as a fallback spelling.
pub const REDIS_ENV: &str = "TEST_REDIS_URL";
pub const REDIS_FALLBACK_ENV: &str = "CHAT_ROOM_REDIS_URL";

const POSTGRES_HINT: &str = "TEST_POSTGRES_ADMIN_URL unset; docker-compose.local.yaml \
     default: postgresql://chatroom:chatroom@127.0.0.1:52735/postgres";
const REDIS_HINT: &str = "TEST_REDIS_URL and CHAT_ROOM_REDIS_URL unset; \
     docker-compose.local.yaml default: redis://127.0.0.1:6379/";

/// One line per skipped test, written straight to the stderr file descriptor so libtest's
/// output capture cannot swallow it. The `SKIPPED: <service> not verified:` prefix is
/// frozen — humans and scripts grep for it (see `docs/devlog/TG-013.md`).
pub fn emit_skip_marker(service: &str, test_name: &str, hint: &str) {
    let line = format!("SKIPPED: {service} not verified: {test_name} — {hint}\n");
    let mut stderr = std::io::stderr().lock();
    let _ = stderr.write_all(line.as_bytes());
    let _ = stderr.flush();
}

/// The admin URL when PostgreSQL is configured, or a visible skip.
pub fn postgres_admin_url_or_skip(test_name: &str) -> Option<String> {
    match std::env::var(POSTGRES_ENV) {
        Ok(url) => Some(url),
        Err(_) => {
            emit_skip_marker("PostgreSQL", test_name, POSTGRES_HINT);
            None
        }
    }
}

/// Connects to the admin database, or skips visibly when PostgreSQL is not configured.
/// A configured-but-unreachable server panics: skipping here would disguise
/// "misconfigured" as "not configured".
pub async fn postgres_admin_pool_or_skip(test_name: &str) -> Option<(String, sqlx::PgPool)> {
    let url = postgres_admin_url_or_skip(test_name)?;
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect(&url)
        .await
        .unwrap_or_else(|error| {
            panic!(
                "{test_name}: {POSTGRES_ENV} is set but PostgreSQL at {url} \
                 is unreachable: {error}"
            )
        });
    Some((url, pool))
}

/// The Redis URL when configured, or a visible skip. Callers connect themselves and
/// already fail loudly on an unreachable server.
pub fn redis_url_or_skip(test_name: &str) -> Option<String> {
    match std::env::var(REDIS_ENV).or_else(|_| std::env::var(REDIS_FALLBACK_ENV)) {
        Ok(url) => Some(url),
        Err(_) => {
            emit_skip_marker("Redis", test_name, REDIS_HINT);
            None
        }
    }
}
