//! Environment overrides used by local Compose and container deployments.

use std::{path::PathBuf, str::FromStr};

use super::AppConfig;

pub(super) fn apply(config: &mut AppConfig) {
    apply_with(config, |name| std::env::var(name).ok());
}

fn apply_with(config: &mut AppConfig, mut value: impl FnMut(&str) -> Option<String>) {
    macro_rules! parse {
        ($target:expr, $name:literal) => {
            set_parsed(&mut $target, value($name));
        };
    }
    macro_rules! string {
        ($target:expr, $name:literal) => {
            set_string(&mut $target, value($name));
        };
    }

    if let Some(usernames) = value("CHAT_ROOM_ADMIN_USERNAMES") {
        config.admin.usernames = usernames
            .split(',')
            .map(str::trim)
            .filter(|username| !username.is_empty())
            .map(str::to_string)
            .collect();
    }
    parse!(
        config.admin.orphan_retention_hours,
        "CHAT_ROOM_ADMIN_ORPHAN_RETENTION_HOURS"
    );
    parse!(
        config.admin.deleted_room_retention_days,
        "CHAT_ROOM_ADMIN_DELETED_ROOM_RETENTION_DAYS"
    );
    parse!(
        config.observability.json_logs,
        "CHAT_ROOM_OBSERVABILITY_JSON_LOGS"
    );
    if let Some(dependencies) = value("CHAT_ROOM_REQUIRED_DEPENDENCIES") {
        config.observability.required_dependencies = dependencies
            .split(',')
            .map(str::trim)
            .filter(|dependency| !dependency.is_empty())
            .map(str::to_string)
            .collect();
    }

    parse!(
        config.uploads.max_file_size_mib,
        "CHAT_ROOM_UPLOADS_MAX_FILE_SIZE_MIB"
    );
    parse!(
        config.uploads.chunk_size_mib,
        "CHAT_ROOM_UPLOADS_CHUNK_SIZE_MIB"
    );
    parse!(
        config.uploads.abandoned_upload_gc_hours,
        "CHAT_ROOM_UPLOADS_ABANDONED_UPLOAD_GC_HOURS"
    );
    set_path(
        &mut config.attachments.directory,
        value("CHAT_ROOM_ATTACHMENTS_DIRECTORY"),
    );
    parse!(config.attachments.oss.enabled, "CHAT_ROOM_OSS_ENABLED");
    parse!(
        config.attachments.oss.local_mirror_enabled,
        "CHAT_ROOM_OSS_LOCAL_MIRROR_ENABLED"
    );
    parse!(
        config.attachments.oss.direct_upload_enabled,
        "CHAT_ROOM_OSS_DIRECT_UPLOAD_ENABLED"
    );
    parse!(
        config.attachments.oss.presign_expiry_secs,
        "CHAT_ROOM_OSS_PRESIGN_EXPIRY_SECS"
    );
    parse!(
        config.attachments.oss.operation_timeout_secs,
        "CHAT_ROOM_OSS_OPERATION_TIMEOUT_SECS"
    );
    string!(
        config.attachments.oss.presign_endpoint,
        "CHAT_ROOM_OSS_PRESIGN_ENDPOINT"
    );
    string!(
        config.attachments.oss.presign_addressing_style,
        "CHAT_ROOM_OSS_PRESIGN_ADDRESSING_STYLE"
    );
    string!(config.attachments.oss.endpoint, "CHAT_ROOM_OSS_ENDPOINT");
    string!(config.attachments.oss.bucket, "CHAT_ROOM_OSS_BUCKET");
    string!(
        config.attachments.oss.access_key_id,
        "CHAT_ROOM_OSS_ACCESS_KEY_ID"
    );
    string!(
        config.attachments.oss.access_key_secret,
        "CHAT_ROOM_OSS_ACCESS_KEY_SECRET"
    );
    string!(config.attachments.oss.root, "CHAT_ROOM_OSS_ROOT");

    string!(config.database.kind, "CHAT_ROOM_DATABASE_KIND");
    set_path(
        &mut config.database.sqlite_path,
        value("CHAT_ROOM_DATABASE_SQLITE_PATH"),
    );
    parse!(
        config.database.max_connections,
        "CHAT_ROOM_DATABASE_MAX_CONNECTIONS"
    );
    if let Some(database_url) = nonempty(value("CHAT_ROOM_DATABASE_URL")) {
        config.database.kind = "postgres".into();
        config.database.postgres_url = database_url;
    }

    if let Some(url) = value("CHAT_ROOM_REDIS_URL") {
        config.redis.enabled = !url.trim().is_empty();
        config.redis.url = url;
    }
    parse!(config.redis.enabled, "CHAT_ROOM_REDIS_ENABLED");
    string!(config.redis.key_prefix, "CHAT_ROOM_REDIS_KEY_PREFIX");
    parse!(
        config.redis.connect_timeout_ms,
        "CHAT_ROOM_REDIS_CONNECT_TIMEOUT_MS"
    );
    parse!(
        config.redis.command_timeout_ms,
        "CHAT_ROOM_REDIS_COMMAND_TIMEOUT_MS"
    );
    parse!(
        config.redis.message_ttl_secs,
        "CHAT_ROOM_REDIS_MESSAGE_TTL_SECS"
    );

    parse!(
        config.work_queue.message_concurrency,
        "CHAT_ROOM_WORK_QUEUE_MESSAGE_CONCURRENCY"
    );
    parse!(
        config.work_queue.upload_concurrency,
        "CHAT_ROOM_WORK_QUEUE_UPLOAD_CONCURRENCY"
    );
    parse!(
        config.work_queue.wait_timeout_secs,
        "CHAT_ROOM_WORK_QUEUE_WAIT_TIMEOUT_SECS"
    );

    super::environment_web_push::apply(config, &mut value);

    parse!(
        config.realtime.poll_interval_ms,
        "CHAT_ROOM_REALTIME_POLL_INTERVAL_MS"
    );
    parse!(
        config.realtime.heartbeat_interval_secs,
        "CHAT_ROOM_REALTIME_HEARTBEAT_INTERVAL_SECS"
    );
    parse!(
        config.realtime.auth_timeout_secs,
        "CHAT_ROOM_REALTIME_AUTH_TIMEOUT_SECS"
    );
    parse!(
        config.realtime.history_replay_limit,
        "CHAT_ROOM_REALTIME_HISTORY_REPLAY_LIMIT"
    );
    parse!(
        config.realtime.message_poll_limit,
        "CHAT_ROOM_REALTIME_MESSAGE_POLL_LIMIT"
    );
    parse!(
        config.realtime.poke_cooldown_secs,
        "CHAT_ROOM_REALTIME_POKE_COOLDOWN_SECS"
    );
    parse!(
        config.auth.session_lifetime_days,
        "CHAT_ROOM_AUTH_SESSION_LIFETIME_DAYS"
    );
    string!(
        config.auth.registration_mode,
        "CHAT_ROOM_AUTH_REGISTRATION_MODE"
    );
    parse!(
        config.auth.rate_limit_window_secs,
        "CHAT_ROOM_AUTH_RATE_LIMIT_WINDOW_SECS"
    );
    parse!(
        config.auth.rate_limit_ip_attempts,
        "CHAT_ROOM_AUTH_RATE_LIMIT_IP_ATTEMPTS"
    );
    parse!(
        config.auth.rate_limit_account_attempts,
        "CHAT_ROOM_AUTH_RATE_LIMIT_ACCOUNT_ATTEMPTS"
    );
    if let Some(origins) = value("CHAT_ROOM_CORS_ALLOWED_ORIGINS") {
        config.security.cors_allowed_origins = origins
            .split(',')
            .map(str::trim)
            .filter(|origin| !origin.is_empty())
            .map(str::to_string)
            .collect();
    }
    parse!(
        config.security.trust_proxy_headers,
        "CHAT_ROOM_TRUST_PROXY_HEADERS"
    );

    parse!(config.ai.enabled, "CHAT_ROOM_AI_ENABLED");
    string!(config.ai.provider, "CHAT_ROOM_AI_PROVIDER");
    string!(config.ai.api_key_env, "CHAT_ROOM_AI_API_KEY_ENV");
    string!(config.ai.model, "CHAT_ROOM_AI_MODEL");
    set_optional_string(&mut config.ai.base_url, value("CHAT_ROOM_AI_BASE_URL"));
    set_optional_string(&mut config.ai.fast_model, value("CHAT_ROOM_AI_FAST_MODEL"));
    set_json(
        &mut config.ai.standard_extra_body,
        value("CHAT_ROOM_AI_STANDARD_EXTRA_BODY"),
    );
    set_json(
        &mut config.ai.reasoning_extra_body,
        value("CHAT_ROOM_AI_REASONING_EXTRA_BODY"),
    );
    parse!(
        config.ai.max_context_messages,
        "CHAT_ROOM_AI_MAX_CONTEXT_MESSAGES"
    );
    parse!(
        config.ai.analysis_context_messages,
        "CHAT_ROOM_AI_ANALYSIS_CONTEXT_MESSAGES"
    );
    parse!(
        config.ai.request_timeout_secs,
        "CHAT_ROOM_AI_REQUEST_TIMEOUT_SECS"
    );
    parse!(
        config.ai.stream_idle_timeout_secs,
        "CHAT_ROOM_AI_STREAM_IDLE_TIMEOUT_SECS"
    );
    parse!(
        config.ai.stream_total_timeout_secs,
        "CHAT_ROOM_AI_STREAM_TOTAL_TIMEOUT_SECS"
    );
    parse!(
        config.ai.suggest_cooldown_secs,
        "CHAT_ROOM_AI_SUGGEST_COOLDOWN_SECS"
    );

    parse!(config.vector_store.enabled, "CHAT_ROOM_VECTOR_ENABLED");
    string!(config.vector_store.url, "CHAT_ROOM_VECTOR_URL");
    string!(
        config.vector_store.collection,
        "CHAT_ROOM_VECTOR_COLLECTION"
    );
    string!(
        config.vector_store.api_key_env,
        "CHAT_ROOM_VECTOR_API_KEY_ENV"
    );
    parse!(
        config.vector_store.dimensions,
        "CHAT_ROOM_EMBEDDING_DIMENSIONS"
    );
    parse!(config.vector_store.top_k, "CHAT_ROOM_VECTOR_TOP_K");
    parse!(
        config.vector_store.score_threshold,
        "CHAT_ROOM_VECTOR_SCORE_THRESHOLD"
    );
    string!(
        config.vector_store.embedding_base_url,
        "CHAT_ROOM_EMBEDDING_BASE_URL"
    );
    string!(
        config.vector_store.embedding_model,
        "CHAT_ROOM_EMBEDDING_MODEL"
    );
    string!(
        config.vector_store.embedding_api_key_env,
        "CHAT_ROOM_EMBEDDING_API_KEY_ENV"
    );
    parse!(
        config.vector_store.rerank_enabled,
        "CHAT_ROOM_RERANK_ENABLED"
    );
    string!(
        config.vector_store.rerank_base_url,
        "CHAT_ROOM_RERANK_BASE_URL"
    );
    string!(config.vector_store.rerank_model, "CHAT_ROOM_RERANK_MODEL");
    string!(
        config.vector_store.rerank_api_key_env,
        "CHAT_ROOM_RERANK_API_KEY_ENV"
    );
    parse!(
        config.vector_store.rerank_timeout_ms,
        "CHAT_ROOM_RERANK_TIMEOUT_MS"
    );
    parse!(
        config.vector_store.rerank_score_threshold,
        "CHAT_ROOM_RERANK_SCORE_THRESHOLD"
    );
    parse!(
        config.vector_store.worker_interval_ms,
        "CHAT_ROOM_VECTOR_WORKER_INTERVAL_MS"
    );
}

fn nonempty(value: Option<String>) -> Option<String> {
    value.filter(|candidate| !candidate.trim().is_empty())
}

fn set_string(target: &mut String, value: Option<String>) {
    if let Some(value) = nonempty(value) {
        *target = value;
    }
}

fn set_path(target: &mut PathBuf, value: Option<String>) {
    if let Some(value) = nonempty(value) {
        *target = value.into();
    }
}

fn set_optional_string(target: &mut Option<String>, value: Option<String>) {
    if let Some(value) = nonempty(value) {
        *target = Some(value);
    }
}

fn set_json(target: &mut Option<serde_json::Value>, value: Option<String>) {
    if let Some(value) = nonempty(value).and_then(|value| serde_json::from_str(&value).ok()) {
        *target = Some(value);
    }
}

fn set_parsed<T: FromStr>(target: &mut T, value: Option<String>) {
    if let Some(value) = value.and_then(|value| value.parse::<T>().ok()) {
        *target = value;
    }
}

#[cfg(test)]
mod tests;
