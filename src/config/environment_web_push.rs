use std::str::FromStr;

use super::AppConfig;

pub(super) fn apply(config: &mut AppConfig, value: &mut impl FnMut(&str) -> Option<String>) {
    set_parsed(
        &mut config.web_push.enabled,
        value("CHAT_ROOM_WEB_PUSH_ENABLED"),
    );
    set_string(
        &mut config.web_push.public_key,
        value("CHAT_ROOM_WEB_PUSH_PUBLIC_KEY"),
    );
    set_string(
        &mut config.web_push.private_key,
        value("CHAT_ROOM_WEB_PUSH_PRIVATE_KEY"),
    );
    set_string(
        &mut config.web_push.subject,
        value("CHAT_ROOM_WEB_PUSH_SUBJECT"),
    );
    if let Some(hosts) = value("CHAT_ROOM_WEB_PUSH_ALLOWED_ENDPOINT_HOSTS") {
        config.web_push.allowed_endpoint_hosts = hosts
            .split(',')
            .map(str::trim)
            .filter(|host| !host.is_empty())
            .map(str::to_owned)
            .collect();
    }
    set_parsed(
        &mut config.web_push.poll_interval_ms,
        value("CHAT_ROOM_WEB_PUSH_POLL_INTERVAL_MS"),
    );
    set_parsed(
        &mut config.web_push.request_timeout_secs,
        value("CHAT_ROOM_WEB_PUSH_REQUEST_TIMEOUT_SECS"),
    );
    set_parsed(
        &mut config.web_push.max_attempts,
        value("CHAT_ROOM_WEB_PUSH_MAX_ATTEMPTS"),
    );
}

fn set_string(target: &mut String, value: Option<String>) {
    if let Some(value) = value.filter(|candidate| !candidate.trim().is_empty()) {
        *target = value;
    }
}

fn set_parsed<T: FromStr>(target: &mut T, value: Option<String>) {
    if let Some(value) = value.and_then(|value| value.parse::<T>().ok()) {
        *target = value;
    }
}
