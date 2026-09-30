//! `CHAT_ROOM_AI_*` overrides for the `[ai]` section.

use super::value::{set_json, set_optional_string, set_parsed, set_string};
use crate::config::AppConfig;

pub(super) fn apply(config: &mut AppConfig, value: &mut impl FnMut(&str) -> Option<String>) {
    let ai = &mut config.ai;
    set_parsed(&mut ai.enabled, value("CHAT_ROOM_AI_ENABLED"));
    set_string(&mut ai.provider, value("CHAT_ROOM_AI_PROVIDER"));
    set_string(&mut ai.api_key_env, value("CHAT_ROOM_AI_API_KEY_ENV"));
    set_string(&mut ai.model, value("CHAT_ROOM_AI_MODEL"));
    set_optional_string(&mut ai.base_url, value("CHAT_ROOM_AI_BASE_URL"));
    set_optional_string(&mut ai.fast_model, value("CHAT_ROOM_AI_FAST_MODEL"));
    set_json(
        &mut ai.standard_extra_body,
        value("CHAT_ROOM_AI_STANDARD_EXTRA_BODY"),
    );
    set_json(
        &mut ai.reasoning_extra_body,
        value("CHAT_ROOM_AI_REASONING_EXTRA_BODY"),
    );
    set_optional_string(&mut ai.vision_model, value("CHAT_ROOM_AI_VISION_MODEL"));
    set_optional_string(
        &mut ai.vision_base_url,
        value("CHAT_ROOM_AI_VISION_BASE_URL"),
    );
    set_string(
        &mut ai.vision_api_key_env,
        value("CHAT_ROOM_AI_VISION_API_KEY_ENV"),
    );
    set_parsed(
        &mut ai.vision_max_images,
        value("CHAT_ROOM_AI_VISION_MAX_IMAGES"),
    );
    set_parsed(
        &mut ai.vision_max_total_images,
        value("CHAT_ROOM_AI_VISION_MAX_TOTAL_IMAGES"),
    );
    set_parsed(
        &mut ai.vision_max_image_mib,
        value("CHAT_ROOM_AI_VISION_MAX_IMAGE_MIB"),
    );
    set_parsed(
        &mut ai.vision_request_timeout_secs,
        value("CHAT_ROOM_AI_VISION_REQUEST_TIMEOUT_SECS"),
    );
    set_parsed(
        &mut ai.max_context_messages,
        value("CHAT_ROOM_AI_MAX_CONTEXT_MESSAGES"),
    );
    set_parsed(
        &mut ai.analysis_context_messages,
        value("CHAT_ROOM_AI_ANALYSIS_CONTEXT_MESSAGES"),
    );
    set_parsed(
        &mut ai.request_timeout_secs,
        value("CHAT_ROOM_AI_REQUEST_TIMEOUT_SECS"),
    );
    set_parsed(
        &mut ai.stream_idle_timeout_secs,
        value("CHAT_ROOM_AI_STREAM_IDLE_TIMEOUT_SECS"),
    );
    set_parsed(
        &mut ai.stream_total_timeout_secs,
        value("CHAT_ROOM_AI_STREAM_TOTAL_TIMEOUT_SECS"),
    );
    set_parsed(
        &mut ai.suggest_cooldown_secs,
        value("CHAT_ROOM_AI_SUGGEST_COOLDOWN_SECS"),
    );
}
