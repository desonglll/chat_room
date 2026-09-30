//! `CHAT_ROOM_VECTOR_*`, `CHAT_ROOM_EMBEDDING_*` and `CHAT_ROOM_RERANK_*`
//! overrides for the `[vector_store]` section.

use super::value::{set_parsed, set_string};
use crate::config::AppConfig;

pub(super) fn apply(config: &mut AppConfig, value: &mut impl FnMut(&str) -> Option<String>) {
    let store = &mut config.vector_store;
    set_parsed(&mut store.enabled, value("CHAT_ROOM_VECTOR_ENABLED"));
    set_string(&mut store.url, value("CHAT_ROOM_VECTOR_URL"));
    set_string(&mut store.collection, value("CHAT_ROOM_VECTOR_COLLECTION"));
    set_string(
        &mut store.api_key_env,
        value("CHAT_ROOM_VECTOR_API_KEY_ENV"),
    );
    set_parsed(
        &mut store.dimensions,
        value("CHAT_ROOM_EMBEDDING_DIMENSIONS"),
    );
    set_parsed(&mut store.top_k, value("CHAT_ROOM_VECTOR_TOP_K"));
    set_parsed(
        &mut store.score_threshold,
        value("CHAT_ROOM_VECTOR_SCORE_THRESHOLD"),
    );
    set_string(
        &mut store.embedding_base_url,
        value("CHAT_ROOM_EMBEDDING_BASE_URL"),
    );
    set_string(
        &mut store.embedding_model,
        value("CHAT_ROOM_EMBEDDING_MODEL"),
    );
    set_string(
        &mut store.embedding_api_key_env,
        value("CHAT_ROOM_EMBEDDING_API_KEY_ENV"),
    );
    set_parsed(&mut store.rerank_enabled, value("CHAT_ROOM_RERANK_ENABLED"));
    set_string(
        &mut store.rerank_base_url,
        value("CHAT_ROOM_RERANK_BASE_URL"),
    );
    set_string(&mut store.rerank_model, value("CHAT_ROOM_RERANK_MODEL"));
    set_string(
        &mut store.rerank_api_key_env,
        value("CHAT_ROOM_RERANK_API_KEY_ENV"),
    );
    set_parsed(
        &mut store.rerank_timeout_ms,
        value("CHAT_ROOM_RERANK_TIMEOUT_MS"),
    );
    set_parsed(
        &mut store.rerank_score_threshold,
        value("CHAT_ROOM_RERANK_SCORE_THRESHOLD"),
    );
    set_parsed(
        &mut store.worker_interval_ms,
        value("CHAT_ROOM_VECTOR_WORKER_INTERVAL_MS"),
    );
}
