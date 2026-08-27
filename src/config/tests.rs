use std::path::PathBuf;

use super::*;

#[test]
fn parses_upload_limit_and_rejects_zero() {
    let config: AppConfig =
        toml::from_str("[uploads]\nmax_file_size_mib = 128\n[attachments]\ndirectory = 'files'")
            .unwrap();
    assert_eq!(config.attachments.directory, PathBuf::from("files"));
    assert_eq!(
        config.validate().unwrap().max_upload_bytes().unwrap(),
        128 * 1024 * 1024
    );

    let invalid: AppConfig = toml::from_str("[uploads]\nmax_file_size_mib = 0").unwrap();
    assert!(invalid.validate().is_err());
}

#[test]
fn defaults_match_the_previous_hardcoded_constants() {
    let config = AppConfig::default();
    assert_eq!(config.realtime.poll_interval_ms, 250);
    assert_eq!(config.realtime.heartbeat_interval_secs, 15);
    assert_eq!(config.realtime.auth_timeout_secs, 10);
    assert_eq!(config.realtime.history_replay_limit, 100);
    assert_eq!(config.realtime.message_poll_limit, 200);
    assert_eq!(config.realtime.poke_cooldown_secs, 5);
    assert_eq!(config.auth.session_lifetime_days, 30);
    assert_eq!(config.auth.registration_mode, "open");
    assert!(config.admin.usernames.is_empty());
    assert_eq!(config.admin.orphan_retention_hours, 168);
    assert_eq!(config.ai.request_timeout_secs, 60);
    assert_eq!(config.ai.stream_idle_timeout_secs, 30);
    assert_eq!(config.ai.stream_total_timeout_secs, 300);
    assert_eq!(config.ai.suggest_cooldown_secs, 10);
    assert_eq!(config.uploads.chunk_size_mib, 8);
    assert_eq!(config.uploads.abandoned_upload_gc_hours, 24);
    assert_eq!(config.redis.message_ttl_secs, 30);
    assert_eq!(config.work_queue.message_concurrency, 32);
    assert_eq!(config.work_queue.upload_concurrency, 4);
    assert_eq!(config.work_queue.wait_timeout_secs, 30);
}

#[test]
fn rejects_invalid_cache_and_work_queue_limits() {
    let invalid_cache: AppConfig = toml::from_str("[redis]\nmessage_ttl_secs = 0").unwrap();
    assert!(invalid_cache.validate().is_err());

    let invalid_queue: AppConfig = toml::from_str("[work_queue]\nupload_concurrency = 0").unwrap();
    assert!(invalid_queue.validate().is_err());
}

#[test]
fn parses_realtime_and_auth_overrides() {
    let parsed: AppConfig = toml::from_str(
        "[realtime]\npoll_interval_ms = 500\nmessage_poll_limit = 50\n[auth]\nsession_lifetime_days = 7\nregistration_mode = 'invite_only'",
    )
    .unwrap();
    let config = parsed.validate().unwrap();
    assert_eq!(config.realtime.poll_interval_ms, 500);
    assert_eq!(config.realtime.message_poll_limit, 50);
    assert_eq!(config.auth.session_lifetime_days, 7);
    assert_eq!(config.auth.registration_mode, "invite_only");
    assert_eq!(config.realtime.heartbeat_interval_secs, 15);

    let invalid: AppConfig = toml::from_str("[auth]\nsession_lifetime_days = 0").unwrap();
    assert!(invalid.validate().is_err());
    let invalid_mode: AppConfig = toml::from_str("[auth]\nregistration_mode = 'private'").unwrap();
    assert!(invalid_mode.validate().is_err());
}

#[test]
fn oss_attachment_backend_is_disabled_by_default_and_validated_when_enabled() {
    let default = AppConfig::default();
    assert!(!default.attachments.oss.enabled);
    assert!(default.validate().is_ok());

    let missing_bucket: AppConfig = toml::from_str(
        "[attachments.oss]\nenabled = true\nendpoint = 'https://oss-cn-hangzhou.aliyuncs.com'\naccess_key_id = 'id'\naccess_key_secret = 'secret'",
    )
    .unwrap();
    assert!(missing_bucket.validate().is_err());

    let complete: AppConfig = toml::from_str(
        "[attachments.oss]\nenabled = true\nlocal_mirror_enabled = true\ndirect_upload_enabled = true\npresign_expiry_secs = 600\nendpoint = 'https://oss-cn-hangzhou.aliyuncs.com'\nbucket = 'my-bucket'\naccess_key_id = 'id'\naccess_key_secret = 'secret'",
    )
    .unwrap();
    assert!(complete.attachments.oss.local_mirror_enabled);
    assert!(complete.attachments.oss.direct_upload_enabled);
    assert!(complete.validate().is_ok());

    let direct_without_oss: AppConfig =
        toml::from_str("[attachments.oss]\ndirect_upload_enabled = true").unwrap();
    assert!(direct_without_oss.validate().is_err());

    let bad_expiry: AppConfig =
        toml::from_str("[attachments.oss]\npresign_expiry_secs = 5").unwrap();
    assert!(bad_expiry.validate().is_err());
}

#[test]
fn ai_config_can_start_without_credentials_but_is_not_ready() {
    let config: AppConfig = toml::from_str(
        "[ai]\nenabled = true\nprovider = 'openai'\napi_key_env = 'CHAT_ROOM_TEST_MISSING_AI_KEY'\nmodel = 'gpt-4o-mini'",
    )
    .unwrap();
    assert!(config.clone().validate().is_ok());
    assert_eq!(
        config.ai.runtime_status_with(|_| None),
        AiRuntimeStatus::MissingCredentials
    );

    let ready: AppConfig = toml::from_str(
        "[ai]\nenabled = true\nprovider = 'openai'\napi_key_env = 'CHAT_ROOM_AI_API_KEY'\nmodel = 'gpt-4o-mini'",
    )
    .unwrap();
    assert_eq!(
        ready
            .ai
            .runtime_status_with(|name| (name == "CHAT_ROOM_AI_API_KEY").then(|| "secret".into())),
        AiRuntimeStatus::Ready
    );

    let missing_model: AppConfig =
        toml::from_str("[ai]\nenabled = true\nprovider = 'openai'\napi_key_env = 'X'\nmodel = ''")
            .unwrap();
    assert!(missing_model.validate().is_err());

    let bad_provider: AppConfig =
        toml::from_str("[ai]\nenabled = true\nprovider = 'bogus'\napi_key_env = 'X'\nmodel = 'm'")
            .unwrap();
    assert!(bad_provider.validate().is_err());
}

#[test]
fn vector_store_is_opt_in_and_requires_a_complete_embedding_profile() {
    let default = AppConfig::default();
    assert!(!default.vector_store.enabled);
    assert!(default.validate().is_ok());

    let incomplete: AppConfig = toml::from_str(
        "[vector_store]\nenabled = true\nurl = 'http://qdrant:6333'\ndimensions = 1024",
    )
    .unwrap();
    assert!(incomplete.validate().is_err());

    let complete: AppConfig = toml::from_str(
        "[vector_store]\nenabled = true\nurl = 'http://qdrant:6333'\ncollection = 'messages'\ndimensions = 1024\nembedding_base_url = 'https://ai.example/v1'\nembedding_model = 'embed-v1'\nembedding_api_key_env = 'EMBEDDING_KEY'",
    )
    .unwrap();
    assert!(complete.validate().is_ok());

    let incomplete_rerank: AppConfig = toml::from_str(
        "[vector_store]\nenabled = true\nurl = 'http://qdrant:6333'\ncollection = 'messages'\ndimensions = 1024\nembedding_base_url = 'https://ai.example/v1'\nembedding_model = 'embed-v1'\nrerank_enabled = true",
    )
    .unwrap();
    assert!(incomplete_rerank.validate().is_err());
}
