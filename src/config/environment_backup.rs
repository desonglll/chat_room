use std::str::FromStr;

use super::AppConfig;

pub(super) fn apply(config: &mut AppConfig, value: &mut impl FnMut(&str) -> Option<String>) {
    set_parsed(
        &mut config.backup.enabled,
        value("CHAT_ROOM_BACKUP_ENABLED"),
    );
    set_parsed(
        &mut config.backup.interval_minutes,
        value("CHAT_ROOM_BACKUP_INTERVAL_MINUTES"),
    );
    set_parsed(
        &mut config.backup.retention_count,
        value("CHAT_ROOM_BACKUP_RETENTION_COUNT"),
    );
    if let Some(target) = nonempty(value("CHAT_ROOM_BACKUP_TARGET_BACKEND")) {
        config.backup.target_backend = target;
    }
    if let Some(directory) = nonempty(value("CHAT_ROOM_BACKUP_DIRECTORY")) {
        config.backup.directory = directory.into();
    }
    set_parsed(
        &mut config.backup.include_files,
        value("CHAT_ROOM_BACKUP_INCLUDE_FILES"),
    );
}

fn nonempty(value: Option<String>) -> Option<String> {
    value.filter(|candidate| !candidate.trim().is_empty())
}

fn set_parsed<T: FromStr>(target: &mut T, value: Option<String>) {
    if let Some(value) = value.and_then(|value| value.parse::<T>().ok()) {
        *target = value;
    }
}
