//! Environment overrides used by local Compose and container deployments.
//!
//! Each `[section]` with more than a handful of variables gets its own submodule
//! so that adding a setting touches one small file. `value.rs` holds the typed
//! assignment primitives they all share.

use self::value::{set_parsed, set_path, set_string};
use super::AppConfig;

mod ai;
mod backup;
#[cfg(test)]
mod tests;
mod value;
mod vector_store;
mod web_push;

pub(super) fn apply(config: &mut AppConfig) {
    apply_with(config, |name| std::env::var(name).ok());
}

fn apply_with(config: &mut AppConfig, mut value: impl FnMut(&str) -> Option<String>) {
    if let Some(usernames) = value("CHAT_ROOM_ADMIN_USERNAMES") {
        config.admin.usernames = comma_separated(&usernames);
    }
    set_parsed(
        &mut config.admin.orphan_retention_hours,
        value("CHAT_ROOM_ADMIN_ORPHAN_RETENTION_HOURS"),
    );
    set_parsed(
        &mut config.admin.deleted_room_retention_days,
        value("CHAT_ROOM_ADMIN_DELETED_ROOM_RETENTION_DAYS"),
    );
    set_parsed(
        &mut config.observability.json_logs,
        value("CHAT_ROOM_OBSERVABILITY_JSON_LOGS"),
    );
    if let Some(dependencies) = value("CHAT_ROOM_REQUIRED_DEPENDENCIES") {
        config.observability.required_dependencies = comma_separated(&dependencies);
    }
    backup::apply(config, &mut value);

    set_parsed(
        &mut config.uploads.max_file_size_mib,
        value("CHAT_ROOM_UPLOADS_MAX_FILE_SIZE_MIB"),
    );
    set_parsed(
        &mut config.uploads.chunk_size_mib,
        value("CHAT_ROOM_UPLOADS_CHUNK_SIZE_MIB"),
    );
    set_parsed(
        &mut config.uploads.abandoned_upload_gc_hours,
        value("CHAT_ROOM_UPLOADS_ABANDONED_UPLOAD_GC_HOURS"),
    );
    set_path(
        &mut config.attachments.directory,
        value("CHAT_ROOM_ATTACHMENTS_DIRECTORY"),
    );
    apply_oss(config, &mut value);
    apply_database(config, &mut value);
    apply_queues(config, &mut value);
    web_push::apply(config, &mut value);
    apply_realtime(config, &mut value);
    apply_auth_and_security(config, &mut value);
    ai::apply(config, &mut value);
    vector_store::apply(config, &mut value);
}

fn apply_oss(config: &mut AppConfig, value: &mut impl FnMut(&str) -> Option<String>) {
    let oss = &mut config.attachments.oss;
    set_parsed(&mut oss.enabled, value("CHAT_ROOM_OSS_ENABLED"));
    set_parsed(
        &mut oss.local_mirror_enabled,
        value("CHAT_ROOM_OSS_LOCAL_MIRROR_ENABLED"),
    );
    set_parsed(
        &mut oss.direct_upload_enabled,
        value("CHAT_ROOM_OSS_DIRECT_UPLOAD_ENABLED"),
    );
    set_parsed(
        &mut oss.presign_expiry_secs,
        value("CHAT_ROOM_OSS_PRESIGN_EXPIRY_SECS"),
    );
    set_parsed(
        &mut oss.operation_timeout_secs,
        value("CHAT_ROOM_OSS_OPERATION_TIMEOUT_SECS"),
    );
    set_string(
        &mut oss.presign_endpoint,
        value("CHAT_ROOM_OSS_PRESIGN_ENDPOINT"),
    );
    set_string(
        &mut oss.presign_addressing_style,
        value("CHAT_ROOM_OSS_PRESIGN_ADDRESSING_STYLE"),
    );
    set_string(&mut oss.endpoint, value("CHAT_ROOM_OSS_ENDPOINT"));
    set_string(&mut oss.bucket, value("CHAT_ROOM_OSS_BUCKET"));
    set_string(&mut oss.access_key_id, value("CHAT_ROOM_OSS_ACCESS_KEY_ID"));
    set_string(
        &mut oss.access_key_secret,
        value("CHAT_ROOM_OSS_ACCESS_KEY_SECRET"),
    );
    set_string(&mut oss.root, value("CHAT_ROOM_OSS_ROOT"));
}

fn apply_database(config: &mut AppConfig, value: &mut impl FnMut(&str) -> Option<String>) {
    set_string(&mut config.database.kind, value("CHAT_ROOM_DATABASE_KIND"));
    set_path(
        &mut config.database.sqlite_path,
        value("CHAT_ROOM_DATABASE_SQLITE_PATH"),
    );
    set_parsed(
        &mut config.database.max_connections,
        value("CHAT_ROOM_DATABASE_MAX_CONNECTIONS"),
    );
    // A single DATABASE_URL is the container convention, so it also selects the
    // adapter rather than only filling in the connection string.
    if let Some(database_url) = value::nonempty(value("CHAT_ROOM_DATABASE_URL")) {
        config.database.kind = "postgres".into();
        config.database.postgres_url = database_url;
    }
}

fn apply_queues(config: &mut AppConfig, value: &mut impl FnMut(&str) -> Option<String>) {
    // An empty CHAT_ROOM_REDIS_URL is how Compose disables Redis, so it is
    // meaningful here even though blank values are ignored elsewhere.
    if let Some(url) = value("CHAT_ROOM_REDIS_URL") {
        config.redis.enabled = !url.trim().is_empty();
        config.redis.url = url;
    }
    set_parsed(&mut config.redis.enabled, value("CHAT_ROOM_REDIS_ENABLED"));
    set_string(
        &mut config.redis.key_prefix,
        value("CHAT_ROOM_REDIS_KEY_PREFIX"),
    );
    set_parsed(
        &mut config.redis.connect_timeout_ms,
        value("CHAT_ROOM_REDIS_CONNECT_TIMEOUT_MS"),
    );
    set_parsed(
        &mut config.redis.command_timeout_ms,
        value("CHAT_ROOM_REDIS_COMMAND_TIMEOUT_MS"),
    );
    set_parsed(
        &mut config.redis.message_ttl_secs,
        value("CHAT_ROOM_REDIS_MESSAGE_TTL_SECS"),
    );

    set_parsed(
        &mut config.work_queue.message_concurrency,
        value("CHAT_ROOM_WORK_QUEUE_MESSAGE_CONCURRENCY"),
    );
    set_parsed(
        &mut config.work_queue.upload_concurrency,
        value("CHAT_ROOM_WORK_QUEUE_UPLOAD_CONCURRENCY"),
    );
    set_parsed(
        &mut config.work_queue.wait_timeout_secs,
        value("CHAT_ROOM_WORK_QUEUE_WAIT_TIMEOUT_SECS"),
    );
}

fn apply_realtime(config: &mut AppConfig, value: &mut impl FnMut(&str) -> Option<String>) {
    let realtime = &mut config.realtime;
    set_parsed(
        &mut realtime.poll_interval_ms,
        value("CHAT_ROOM_REALTIME_POLL_INTERVAL_MS"),
    );
    set_parsed(
        &mut realtime.heartbeat_interval_secs,
        value("CHAT_ROOM_REALTIME_HEARTBEAT_INTERVAL_SECS"),
    );
    set_parsed(
        &mut realtime.auth_timeout_secs,
        value("CHAT_ROOM_REALTIME_AUTH_TIMEOUT_SECS"),
    );
    set_parsed(
        &mut realtime.history_replay_limit,
        value("CHAT_ROOM_REALTIME_HISTORY_REPLAY_LIMIT"),
    );
    set_parsed(
        &mut realtime.message_poll_limit,
        value("CHAT_ROOM_REALTIME_MESSAGE_POLL_LIMIT"),
    );
    set_parsed(
        &mut realtime.poke_cooldown_secs,
        value("CHAT_ROOM_REALTIME_POKE_COOLDOWN_SECS"),
    );
}

fn apply_auth_and_security(config: &mut AppConfig, value: &mut impl FnMut(&str) -> Option<String>) {
    set_parsed(
        &mut config.auth.session_lifetime_days,
        value("CHAT_ROOM_AUTH_SESSION_LIFETIME_DAYS"),
    );
    set_string(
        &mut config.auth.registration_mode,
        value("CHAT_ROOM_AUTH_REGISTRATION_MODE"),
    );
    set_parsed(
        &mut config.auth.rate_limit_window_secs,
        value("CHAT_ROOM_AUTH_RATE_LIMIT_WINDOW_SECS"),
    );
    set_parsed(
        &mut config.auth.rate_limit_ip_attempts,
        value("CHAT_ROOM_AUTH_RATE_LIMIT_IP_ATTEMPTS"),
    );
    set_parsed(
        &mut config.auth.rate_limit_account_attempts,
        value("CHAT_ROOM_AUTH_RATE_LIMIT_ACCOUNT_ATTEMPTS"),
    );
    if let Some(origins) = value("CHAT_ROOM_CORS_ALLOWED_ORIGINS") {
        config.security.cors_allowed_origins = comma_separated(&origins);
    }
    set_parsed(
        &mut config.security.trust_proxy_headers,
        value("CHAT_ROOM_TRUST_PROXY_HEADERS"),
    );
}

/// Comma-separated lists replace the configured list wholesale, including with
/// an empty value — that is how a deployment clears a list from TOML.
fn comma_separated(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(str::to_owned)
        .collect()
}
