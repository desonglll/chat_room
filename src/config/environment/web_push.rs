//! `CHAT_ROOM_WEB_PUSH_*` overrides for the `[web_push]` section.

use super::value::{set_parsed, set_string};
use crate::config::AppConfig;

pub(super) fn apply(config: &mut AppConfig, value: &mut impl FnMut(&str) -> Option<String>) {
    let web_push = &mut config.web_push;
    set_parsed(&mut web_push.enabled, value("CHAT_ROOM_WEB_PUSH_ENABLED"));
    set_string(
        &mut web_push.public_key,
        value("CHAT_ROOM_WEB_PUSH_PUBLIC_KEY"),
    );
    set_string(
        &mut web_push.private_key,
        value("CHAT_ROOM_WEB_PUSH_PRIVATE_KEY"),
    );
    set_string(&mut web_push.subject, value("CHAT_ROOM_WEB_PUSH_SUBJECT"));
    if let Some(hosts) = value("CHAT_ROOM_WEB_PUSH_ALLOWED_ENDPOINT_HOSTS") {
        web_push.allowed_endpoint_hosts = hosts
            .split(',')
            .map(str::trim)
            .filter(|host| !host.is_empty())
            .map(str::to_owned)
            .collect();
    }
    set_parsed(
        &mut web_push.poll_interval_ms,
        value("CHAT_ROOM_WEB_PUSH_POLL_INTERVAL_MS"),
    );
    set_parsed(
        &mut web_push.request_timeout_secs,
        value("CHAT_ROOM_WEB_PUSH_REQUEST_TIMEOUT_SECS"),
    );
    set_parsed(
        &mut web_push.max_attempts,
        value("CHAT_ROOM_WEB_PUSH_MAX_ATTEMPTS"),
    );
}
