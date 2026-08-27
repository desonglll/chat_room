use super::*;

fn from(values: &[(&str, &str)]) -> AppConfig {
    let mut config = AppConfig::default();
    apply_with(&mut config, |name| {
        values
            .iter()
            .find_map(|(key, value)| (*key == name).then(|| (*value).to_owned()))
    });
    config
}

#[test]
fn ai_and_vector_settings_can_come_from_the_environment() {
    let config = from(&[
        ("CHAT_ROOM_AI_ENABLED", "true"),
        ("CHAT_ROOM_AI_MODEL", "model-from-env"),
        ("CHAT_ROOM_AI_BASE_URL", "https://ai.example/v1"),
        ("CHAT_ROOM_DATABASE_URL", "postgres://db/chatroom"),
        ("CHAT_ROOM_VECTOR_ENABLED", "true"),
        ("CHAT_ROOM_EMBEDDING_MODEL", "embed-from-env"),
        ("CHAT_ROOM_EMBEDDING_BASE_URL", "https://embed.example/v1"),
        ("CHAT_ROOM_EMBEDDING_DIMENSIONS", "768"),
        ("CHAT_ROOM_RERANK_ENABLED", "true"),
        ("CHAT_ROOM_RERANK_BASE_URL", "https://rerank.example/v1"),
        ("CHAT_ROOM_RERANK_MODEL", "rerank-from-env"),
    ]);

    assert!(config.ai.enabled);
    assert_eq!(config.database.kind, "postgres");
    assert_eq!(config.database.postgres_url, "postgres://db/chatroom");
    assert_eq!(config.ai.model, "model-from-env");
    assert_eq!(config.ai.base_url.as_deref(), Some("https://ai.example/v1"));
    assert!(config.vector_store.enabled);
    assert_eq!(config.vector_store.embedding_model, "embed-from-env");
    assert_eq!(config.vector_store.dimensions, 768);
    assert!(config.vector_store.rerank_enabled);
    assert_eq!(config.vector_store.rerank_model, "rerank-from-env");
}

#[test]
fn all_runtime_sections_can_come_from_the_environment() {
    let config = from(&[
        ("CHAT_ROOM_UPLOADS_MAX_FILE_SIZE_MIB", "2048"),
        ("CHAT_ROOM_ATTACHMENTS_DIRECTORY", "/data/attachments"),
        ("CHAT_ROOM_OSS_ENABLED", "true"),
        ("CHAT_ROOM_OSS_ENDPOINT", "https://oss.example"),
        ("CHAT_ROOM_OSS_BUCKET", "chat-files"),
        ("CHAT_ROOM_OSS_ACCESS_KEY_ID", "access-key"),
        ("CHAT_ROOM_OSS_ACCESS_KEY_SECRET", "secret-key"),
        ("CHAT_ROOM_DATABASE_MAX_CONNECTIONS", "24"),
        ("CHAT_ROOM_AI_API_KEY_ENV", "CUSTOM_AI_KEY"),
        ("CHAT_ROOM_AI_REQUEST_TIMEOUT_SECS", "90"),
        ("CHAT_ROOM_VECTOR_COLLECTION", "messages_v2"),
        ("CHAT_ROOM_VECTOR_SCORE_THRESHOLD", "0.7"),
        ("CHAT_ROOM_REALTIME_POLL_INTERVAL_MS", "500"),
        ("CHAT_ROOM_AUTH_SESSION_LIFETIME_DAYS", "14"),
        ("CHAT_ROOM_AUTH_REGISTRATION_MODE", "disabled"),
        ("CHAT_ROOM_REDIS_KEY_PREFIX", "chat-test"),
        ("CHAT_ROOM_WORK_QUEUE_MESSAGE_CONCURRENCY", "12"),
        ("CHAT_ROOM_ADMIN_ORPHAN_RETENTION_HOURS", "72"),
    ]);

    assert_eq!(config.uploads.max_file_size_mib, 2048);
    assert_eq!(
        config.attachments.directory.to_str(),
        Some("/data/attachments")
    );
    assert!(config.attachments.oss.enabled);
    assert_eq!(config.attachments.oss.endpoint, "https://oss.example");
    assert_eq!(config.attachments.oss.bucket, "chat-files");
    assert_eq!(config.attachments.oss.access_key_id, "access-key");
    assert_eq!(config.attachments.oss.access_key_secret, "secret-key");
    assert_eq!(config.database.max_connections, 24);
    assert_eq!(config.ai.api_key_env, "CUSTOM_AI_KEY");
    assert_eq!(config.ai.request_timeout_secs, 90);
    assert_eq!(config.vector_store.collection, "messages_v2");
    assert_eq!(config.vector_store.score_threshold, 0.7);
    assert_eq!(config.realtime.poll_interval_ms, 500);
    assert_eq!(config.auth.session_lifetime_days, 14);
    assert_eq!(config.auth.registration_mode, "disabled");
    assert_eq!(config.redis.key_prefix, "chat-test");
    assert_eq!(config.work_queue.message_concurrency, 12);
    assert_eq!(config.admin.orphan_retention_hours, 72);
}
